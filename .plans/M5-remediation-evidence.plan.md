# Plan: M5 Remediation Evidence Replay and Regression Forensics

Status: Approved

## Goal

Connect diagprint's existing guarded remediation, semantic report deltas, and
tamper-evident diagnostic history into one verifiable remediation-evidence
layer.

M5 must answer questions such as:

```text
Was this exact remediation transaction associated with these exact history runs?
What happened to this logical diagnostic immediately afterward?
Was the transaction applied or post-apply verified?
Did the same canonical diagnostic later reappear?
Can the full evidence chain be replayed and re-verified without mutating files?
```

The central product behavior is **evidence replay**, not edit replay.

`diagprint replay` reconstructs and verifies historical remediation evidence. It
never reapplies a `FixPlan`, never writes source files, and never executes
commands.

## Non-goals

M5 will not:

- automatically reapply historical edits;
- deserialize an old `FixPlanDescriptor` and execute it;
- execute shell commands;
- infer that a remediation caused a diagnostic to resolve;
- infer that a later recurrence has the same root cause;
- call every post-remediation reappearance a verified regression;
- change canonical-v1 identity;
- change history-v2 run persistence;
- change remediation-receipt-v1 semantics;
- change fix-plan-descriptor-v1 semantics;
- change M4 relationship graph schemas;
- put remediation artifacts into the diagnostic relationship graph;
- create a second diagnostic-history implementation;
- add a new dependency;
- add ML/confidence scoring;
- change the Rust 1.85 MSRV.

M5 v1 also does not add a generic CLI importer for arbitrary receipt JSON.
Evidence is recorded through the typed Rust API from a real
`RemediationReceipt`.

## Context

The required primitives already exist.

### Guarded remediation

`FixPlan` provides:

- machine-applicability enforcement;
- deterministic preconditions;
- guarded multi-file edits;
- transactional writes;
- rollback-on-error;
- deterministic post-apply `FileCheck` verification;
- rollback when verification fails.

`FixPlanDescriptor` provides a deterministic serializable description of the
remediation intent.

### Remediation receipt

`RemediationReceipt` already binds:

```text
before report digest
after report digest
exact fix-plan descriptor digest
plan shape counts
apply / verified status
changed-file count
verification result
semantic delta counts
introduced error count
severity increase count
```

The receipt deliberately excludes source paths, source text, replacement text,
expected edit contents, and verification payloads.

### Semantic delta

`DiagnosticDelta` already classifies diagnostic instances as:

```text
new
resolved
persisting
changed
```

using canonical logical fingerprints and canonical content digests while
preserving duplicate diagnostic instances.

### Diagnostic history

`DiagnosticHistory` already provides:

- immutable run files;
- a hash chain through `previous_run_digest`;
- a mutable checked head;
- report digests;
- privacy-light observations containing fingerprint, digest, and severity;
- semantic transitions;
- per-fingerprint lineage;
- complete re-verification from disk.

M5 composes these systems. It does not replace them.

## Architecture placement

Add two focused core modules:

```text
src/remediation_evidence.rs
src/remediation_replay.rs
```

### remediation_evidence

Owns:

- history-bound remediation evidence schema;
- deterministic evidence-record digest;
- persistence under a history directory;
- append-only/idempotent conflict behavior;
- receipt-to-history transition validation;
- evidence-record loading and verification.

### remediation_replay

Owns:

- per-fingerprint replay view;
- immediate post-remediation diagnostic state;
- later reappearance analysis;
- verified-regression classification;
- evidence-only assessment fields;
- deterministic JSON-ready replay model.

### Existing modules remain authoritative

```text
FixPlan / FixPlanDescriptor
    remediation execution intent

RemediationReceipt
    exact successful transaction + before/after semantic effect

DiagnosticHistory
    longitudinal diagnostic state

DiagnosticRelationshipGraph
    diagnostic-to-diagnostic relationships

M5 remediation evidence
    transaction-to-history transition binding and replay
```

Do not overload M4 with non-diagnostic artifact nodes.

## Data flow

Expected producer flow:

```text
before DiagnosticReport
        │
        ├── append history run N
        │
        ▼
     FixPlan
        │
        ▼
FixPlan::apply()
        │
        ▼
after DiagnosticReport
        │
        ├── append history run N+1
        │
        ▼
RemediationReceipt::from_successful_apply(...)
        │
        ▼
DiagnosticHistory::record_remediation_evidence(
    N,
    N+1,
    &receipt,
)
        │
        ▼
diagprint.remediation.evidence/v1
```

Expected forensic flow:

```text
DiagnosticHistory::open(...)
        │
        ├── verify history-v2 chain
        ├── load remediation sidecars
        ├── verify record digests
        ├── verify exact run digests
        ├── verify exact report digests
        ├── verify receipt-summary / history-transition equality
        │
        ▼
DiagnosticHistory::remediation_replay(fingerprint)
        │
        ▼
diagprint.forensics.remediation-replay/v1
        │
        ├── text
        └── JSON
```

## Invariants

### Exact adjacent transition

A remediation evidence record binds exactly two **adjacent** history runs:

```text
after_run == before_run + 1
```

This is required because the receipt claims one before/after report transition.
Allowing unrecorded intermediate history runs would weaken that association.

### History chain first

Before recording or replaying remediation evidence:

```text
DiagnosticHistory::verify()
```

must succeed.

A remediation sidecar cannot make a broken history chain trustworthy.

### Exact run binding

The record stores:

```text
before_run
after_run
before_run_digest
after_run_digest
before_report_digest
after_report_digest
```

Every field must match the current verified history.

### Exact receipt binding

The evidence record stores a deterministic digest of the serialized
`RemediationReceipt`.

The receipt's:

```text
before_report
after_report
```

must exactly match the selected history runs.

### Exact semantic-effect binding

The receipt's aggregate effect must exactly equal the semantic transition
derived from the two history runs:

```text
before_diagnostics
after_diagnostics
new
resolved
persisting
changed
introduced_errors
severity_increases
```

If any count differs, evidence recording fails.

This prevents a receipt for one scan pair from being attached to a different
history transition even when run labels look similar.

### Receipt status is evidence, not causal proof

Preserve the distinction:

```text
Applied
Verified
```

`Verified` means the FixPlan's declared deterministic post-apply checks passed.
It does **not** prove that the remediation caused every diagnostic change.

### Per-diagnostic observed state

For one canonical fingerprint, replay derives the immediate transition from
history observations:

```text
introduced
resolved
persisting
changed
absent
```

Duplicate instances remain meaningful; classification uses multiset semantics
where required.

### Verified regression

The phrase `regression_after_verified_remediation` is allowed only when all of
these are true:

1. the remediation receipt status is `Verified`;
2. the fingerprint was observed in `before_run`;
3. the fingerprint is absent in `after_run`;
4. the same canonical fingerprint is observed in a later history run.

This is evidence that the same logical diagnostic reappeared after an observed
resolution associated with a verified remediation transaction.

It is **not** evidence that:

- the remediation originally caused the resolution;
- the later diagnostic has the same root cause;
- a particular Git commit caused the recurrence.

For an `Applied` receipt with no verification checks, later recurrence must be
reported as reappearance after an applied remediation, not as a verified
regression.

### Replay is read-only

The replay engine and CLI must not call:

```text
FixPlan::apply
Fixer mutation APIs
filesystem write helpers
shell commands
```

Replay reads history and remediation sidecars only.

### No artifact-to-diagnostic graph coercion

A FixPlan, receipt, or remediation record is not a diagnostic fingerprint.

M5 must not invent fake diagnostic nodes to place remediation artifacts into M4.

## Schema / persistence

### Evidence record

New immutable schema:

```text
diagprint.remediation.evidence/v1
```

Proposed public type:

```text
RemediationEvidenceRecord
```

Proposed fields:

```text
schema

before_run
after_run

before_run_digest
after_run_digest

before_report_digest
after_report_digest

receipt_schema
receipt_digest

plan_descriptor_digest
plan_descriptor_byte_length
plan_applicability

remediation_status
changed_files
verification_checks
verification_passed

effect:
  before_diagnostics
  after_diagnostics
  new
  resolved
  persisting
  changed
  introduced_errors
  severity_increases

record_digest
```

Do not persist:

```text
FixPlan title
FixPlan explanation
source paths
changed file paths
source text
replacement text
expected edit contents
verification text/payloads
diagnostic messages
help
notes
causes
arbitrary attributes
```

Although `RemediationReceipt` is already privacy-conscious, M5 sidecars should
retain only the minimum fields needed for longitudinal verification.

### Record digest

`record_digest` is the `ArtifactDigest` of deterministic compact JSON containing
every record field except `record_digest` itself.

### Persistence path

Store one record per post-remediation transition:

```text
<HISTORY>/remediation/run-XXXXXX.json
```

where `XXXXXX` is `after_run`.

Rules:

- create-new semantics;
- identical retry is idempotent;
- conflicting replacement is rejected;
- filename run index must equal the record's `after_run`;
- unsupported schemas are rejected;
- tampered record digests are rejected.

M5 does not alter history run files or `head.json`.

### Replay schema

New derived schema:

```text
diagprint.forensics.remediation-replay/v1
```

Proposed public types:

```text
DiagnosticRemediationReplay
DiagnosticRemediationReplayStep
DiagnosticRemediationState
DiagnosticRemediationAssessment
```

Replay output is derived from verified history plus verified remediation
evidence records. It is not separately persisted in v1.

## Public API / CLI

### Evidence API

Proposed methods:

```rust
DiagnosticHistory::record_remediation_evidence(
    before_run: usize,
    after_run: usize,
    receipt: &RemediationReceipt,
) -> Result<RemediationEvidenceRecord, RemediationEvidenceError>

DiagnosticHistory::remediation_evidence(
    after_run: usize,
) -> Result<Option<RemediationEvidenceRecord>, RemediationEvidenceError>

DiagnosticHistory::remediation_evidence_records(
) -> Result<Vec<RemediationEvidenceRecord>, RemediationEvidenceError>

DiagnosticHistory::verify_remediation_evidence(
) -> Result<(), RemediationEvidenceError>
```

Exact naming may be refined without changing responsibilities.

### Replay API

Proposed:

```rust
DiagnosticHistory::remediation_replay(
    fingerprint: &str,
) -> Result<DiagnosticRemediationReplay, RemediationReplayError>
```

The core API requires an exact canonical fingerprint.

CLI may reuse existing full-or-unique-prefix resolution behavior.

### CLI

Add:

```text
diagprint replay <HISTORY> <FINGERPRINT> [--format text|json]
```

Alias:

```text
diagprint history replay <HISTORY> <FINGERPRINT> [--format text|json]
```

Also add evidence verification:

```text
diagprint history remediation-verify <HISTORY>
```

Replay text should make evidence boundaries explicit.

Example shape:

```text
DIAGNOSTIC REMEDIATION REPLAY
schema: diagprint.forensics.remediation-replay/v1
fingerprint: diagprint.canonical/v1:sha256:...
history-chain: VERIFIED
remediation-records: VERIFIED

STEP 01
  transition: 000010 -> 000011
  remediation-status: verified
  plan-digest: sha256:...
  before: active instances=1
  after: absent instances=0
  observed-transition: resolved
  later-reappearance: run=000015

ASSESSMENT
  remediation-transaction: VERIFIED
  observed-resolution: ESTABLISHED
  regression-after-verified-remediation: OBSERVED
  remediation-caused-resolution: NOT ESTABLISHED
  recurrence-root-cause: NOT ESTABLISHED
  git-causation: NOT ESTABLISHED
```

JSON carries the same semantic fields without presentation-only formatting.

### No CLI mutation

There is intentionally no:

```text
diagprint replay --apply
diagprint replay --execute
```

in M5 v1.

## Privacy analysis

M5 evidence persistence should be at least as privacy-light as diagnostic
history.

Persist only:

- run indexes;
- cryptographic digests;
- applicability classification;
- transaction status;
- aggregate counts;
- verification count/result;
- changed-file count.

Do not persist:

- source or changed-file paths;
- source contents;
- replacement contents;
- expected-before edit guards;
- verification payload text;
- FixPlan title/explanation;
- diagnostic messages;
- source labels;
- help/notes/causes;
- environment variables;
- Git author/email/remote;
- arbitrary attributes.

Privacy tests must use obvious secret sentinel strings for:

```text
path
old source text
replacement text
verification value
plan title
diagnostic message
```

and prove none appear in the remediation evidence sidecar.

Replay inherits the same privacy boundary because it is derived from history
observations and evidence records.

## Compatibility analysis

M5 should remain additive.

Protected contracts:

```text
diagprint.canonical/v1
diagprint.history.run/v2
diagprint.history.head/v1
diagprint.remediation.receipt/v1
diagprint.fix-plan.descriptor/v1
diagprint.relationship.graph/v1
diagprint.relationship.snapshot/v1
diagprint.forensics.git-provenance/v1
diagprint.capsule/v1
```

Do not change their meaning or serialized fields.

No new dependency is expected.

Rust 1.85 remains the MSRV.

`RemediationReceipt` remains the transaction-level artifact. M5 adds a separate
history-binding layer rather than widening remediation-receipt-v1.

Capsule anchoring of M5 evidence is deferred unless implementation discovers an
existing schema-preserving path that needs no capsule-v1 change. Any broader
capsule work requires plan revision.

## Expected file boundary

Expected new files:

```text
src/remediation_evidence.rs
src/remediation_replay.rs
tests/remediation_evidence.rs
tests/remediation_replay.rs
tests/remediation_replay_cli.rs
docs/forensics-remediation-replay-v1.md
```

Expected modified files:

```text
src/lib.rs
src/bin/diagprint.rs
README.md
CHANGELOG.md
docs/ecosystem-bridges-roadmap.md
.plans/M5-remediation-evidence.plan.md
```

Protected unless this plan is explicitly revised:

```text
Cargo.toml
Cargo.lock

src/canonical.rs
src/fingerprint.rs
src/history.rs
src/fixplan.rs
src/remediation.rs
src/remediation_receipt.rs
src/capsule.rs
src/relationship.rs
src/git_provenance.rs

crates/
```

If the implementation cannot use the existing public APIs of these protected
modules, stop and revise the plan instead of silently changing the boundary.

## Test-first matrix

### Valid evidence binding

Create two adjacent history runs and a real successful `FixPlan` receipt.

Prove:

- record creation succeeds;
- before/after run indexes are exact;
- run digests match;
- report digests match;
- receipt digest is deterministic;
- plan descriptor digest is retained;
- aggregate effect matches the history transition;
- record digest verifies.

### Adjacent-run requirement

Prove recording:

```text
run 0 -> run 2
```

is rejected even when report digests could otherwise be supplied.

### Receipt/report mismatch

Attempt to bind a receipt whose before or after report digest differs from the
selected history transition.

Reject it.

### Aggregate-effect mismatch

Construct a receipt/evidence scenario where semantic counts do not match the
history transition.

Reject it.

### Tamper detection

Persist a valid record, then modify:

- run digest;
- receipt digest;
- plan digest;
- effect count;
- status.

Loading/verification must fail.

### Append-only conflict behavior

Prove:

- first persistence succeeds;
- exact retry is idempotent;
- conflicting record for the same `after_run` is rejected.

### Privacy

Use secret sentinel values in:

- source path;
- old source contents;
- replacement text;
- verification expected text;
- plan title;
- diagnostic message.

Serialize the persisted evidence record and prove every secret is absent.

### Immediate diagnostic state

For one fingerprint, test:

```text
introduced
resolved
persisting
changed
absent
```

with duplicate-instance coverage.

### Verified regression

History shape:

```text
run 0  target active
run 1  target absent     verified remediation 0 -> 1
run 2  target absent
run 3  target active
```

Prove replay reports:

```text
observed transition: resolved
later reappearance: run 3
regression after verified remediation: observed
```

### Applied but unverified recurrence

Repeat with a receipt status of `Applied`.

Prove the later observation is reported, but:

```text
regression_after_verified_remediation
```

is not asserted.

### No false regression while persisting

If the fingerprint remains active immediately after remediation, later activity
is persistence, not a resolved-then-regressed lifecycle.

### Multiple remediation records

Bind more than one remediation transition affecting the same logical
fingerprint.

Replay must be chronological, deterministic, and preserve every verified step.

### Evidence verification before presentation

Tamper with either:

- history chain;
- remediation evidence sidecar.

Both API replay and CLI replay must fail before presenting forensic conclusions.

### CLI prefix resolution

Reuse existing CLI semantics:

- exact fingerprint;
- unique prefix;
- ambiguous prefix rejection;
- missing fingerprint rejection.

### CLI text doctrine

Text output must include explicit boundaries such as:

```text
remediation-caused-resolution: NOT ESTABLISHED
recurrence-root-cause: NOT ESTABLISHED
git-causation: NOT ESTABLISHED
```

### JSON determinism

Same verified history/evidence input must produce semantically identical JSON
regardless of directory iteration order.

### Replay is non-mutating

Snapshot relevant source/history files before replay and prove replay leaves
them byte-identical.

### Protected schema regression

Existing canonical/history/remediation/fix-plan/relationship/Git/capsule tests
remain green without schema updates.

## Implementation sequence

### M5A — history-bound remediation evidence

Add:

- `diagprint.remediation.evidence/v1`;
- `RemediationEvidenceRecord`;
- deterministic record digest;
- receipt digest binding;
- exact adjacent run/report binding;
- semantic-effect equality checks;
- append-only persistence;
- load/verify APIs;
- privacy tests;
- tamper/conflict tests;
- format documentation.

Gate:

```bash
cargo test --test remediation_evidence --no-default-features
cargo test --test remediation_receipt --no-default-features
./scripts/gate.sh forensics
```

Commit and verify exact CI before M5B.

### M5B — forensic replay / regression

Add:

- `diagprint.forensics.remediation-replay/v1`;
- replay view types;
- per-fingerprint immediate transition classification;
- later-reappearance detection;
- verified-regression semantics;
- read-only `diagprint replay`;
- `diagprint history replay` alias;
- `diagprint history remediation-verify`;
- text and JSON formats;
- CLI/API tests;
- README/CHANGELOG/roadmap documentation.

Gate:

```bash
cargo test --test remediation_replay --no-default-features
cargo test --test remediation_replay_cli --no-default-features
./scripts/gate.sh forensics
./scripts/gate.sh full
```

Commit and verify exact CI.

### Exact CI checkpoint

After every implementation checkpoint:

```text
push exact commit
verify GitHub CI success on that SHA
record commit/run in this plan
```

Close M5 only after exact final CI succeeds.

## Failure modes

M5 must fail closed on:

- broken history-v2 chain;
- missing before/after run;
- non-adjacent transition;
- run-digest mismatch;
- report-digest mismatch;
- receipt/report mismatch;
- receipt semantic-effect mismatch;
- unsupported evidence schema;
- evidence filename/index mismatch;
- evidence record digest mismatch;
- conflicting replacement record;
- invalid canonical fingerprint;
- ambiguous CLI fingerprint prefix.

A failed or unverifiable evidence record must never be silently skipped when it
could affect replay output.

## Quality gates

- ./scripts/gate.sh precommit
- ./scripts/gate.sh fast
- ./scripts/gate.sh forensics
- ./scripts/gate.sh full

Additionally:

```bash
cargo test --test remediation_receipt --no-default-features
cargo test --test remediation_evidence --no-default-features
cargo test --test remediation_replay --no-default-features
cargo test --test remediation_replay_cli --no-default-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
```

## Acceptance criteria

M5 is complete when:

- history-bound remediation evidence has a stable v1 schema;
- evidence records are deterministically digested;
- records bind exact adjacent history run digests;
- records bind exact before/after report digests;
- records bind the exact remediation receipt digest;
- receipt semantic effect must equal the history transition;
- evidence persistence is append-only and idempotent for identical retries;
- conflicting replacement is rejected;
- privacy tests prove plan/source/diagnostic secrets are absent;
- replay verifies history and all relevant evidence before analysis;
- replay classifies immediate target state deterministically;
- replay detects later reappearance of the same canonical fingerprint;
- verified regression is only asserted under the strict verified-resolution-
  then-reappearance rule;
- applied-but-unverified remediation is never upgraded to verified regression;
- replay makes no causal claim about resolution or recurrence root cause;
- `diagprint replay` is read-only;
- text and JSON replay surfaces are covered;
- canonical-v1 remains unchanged;
- history-v2 remains unchanged;
- remediation-receipt-v1 remains unchanged;
- fix-plan-descriptor-v1 remains unchanged;
- M4 relationship schemas remain unchanged;
- Git-provenance-v1 remains unchanged;
- capsule-v1 remains unchanged;
- no new dependency is added;
- Rust 1.85 MSRV remains green;
- exact M5A and final M5B CI both succeed.

## Completion record

```text
M5A evidence commit:
M5A CI run:
M5A CI result:

Final M5B replay commit:
Final CI run:
Final CI result:

Notes:
```
