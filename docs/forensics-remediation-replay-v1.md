# Remediation Evidence and Replay v1

M5 connects successful guarded remediation to diagprint's tamper-evident
diagnostic history.

The governing principle is:

```text
replay evidence, not edits
```

M5 never re-executes an historical `FixPlan`.

## Remediation evidence schema

M5A defines:

```text
diagprint.remediation.evidence/v1
```

One evidence record binds:

- one exact successful remediation receipt;
- one exact pre-remediation history run;
- the immediately adjacent post-remediation history run;
- both history run digests;
- both canonical report digests;
- the exact remediation receipt digest;
- the exact fix-plan descriptor digest carried by that receipt;
- remediation status and verification summary;
- semantic before/after diagnostic effect.

Records are stored at:

```text
<HISTORY>/remediation/run-XXXXXX.json
```

where `XXXXXX` is the post-remediation run index.

## Exact-transition rule

Evidence is valid only for adjacent runs:

```text
before_run + 1 == after_run
```

The receipt's before/after report identities must exactly equal the report
identities in those two history runs.

The receipt's aggregate semantic effect must also exactly equal the transition
recomputed from history.

## Digest model

The exact compact JSON serialization of the typed `RemediationReceipt` is
hashed and stored as `receipt_digest`.

The evidence record itself has a separate `record_digest` covering every
evidence field except `record_digest`.

This lets history retain a privacy-light anchor to the exact receipt without
embedding the receipt itself.

## Persistence behavior

Evidence sidecars are append-only.

- first write uses create-new semantics;
- an identical retry is idempotent;
- a different record for the same post-remediation run is rejected;
- tampered record digests are rejected;
- history is re-verified before evidence is recorded or replayed.

## Privacy boundary

The v1 evidence sidecar does not persist:

- fix-plan title or explanation;
- source paths;
- changed-file paths;
- source text;
- edit replacement text;
- expected-before edit guards;
- verification payload text;
- diagnostic messages;
- help or notes;
- causes;
- arbitrary diagnostic attributes.

The record retains cryptographic identities, classifications, verification
summary, changed-file count, and aggregate diagnostic effects.

## M5B replay

M5B will derive:

```text
diagprint.forensics.remediation-replay/v1
```

from verified history plus verified remediation evidence.

Replay is read-only and will never invoke `FixPlan::apply`.

## Forensic replay schema

M5B defines:

```text
diagprint.forensics.remediation-replay/v1
```

Replay is derived, not persisted.

For one exact canonical diagnostic fingerprint, each verified remediation
record becomes one chronological replay step containing:

- before and after history run;
- remediation status;
- verification count;
- receipt, plan descriptor, and evidence record digests;
- target diagnostic instance counts before and after;
- observed transition;
- optional first later reappearance run;
- explicit evidence assessment.

Observed transitions are:

```text
introduced
resolved
persisting
changed
absent
```

`persisting` requires the sorted multiset of canonical diagnostic content
digests to remain exactly equal. A multiplicity or content change under the
same logical fingerprint is therefore `changed`.

## Verified regression rule

`regression_after_verified_remediation` is true only when:

1. the evidence record is valid;
2. remediation status is `verified`, meaning declared post-apply checks existed
   and passed;
3. the target fingerprint is active before the remediation;
4. it is absent immediately afterward;
5. the same canonical fingerprint appears in a later history run.

An `applied` remediation with no declared post-apply checks can still show an
observed resolution and later reappearance, but it is not labeled a
verified-remediation regression.

## Causation doctrine

Replay always keeps these independent conclusions false in v1:

```text
remediation_caused_resolution_established
recurrence_root_cause_established
git_causation_established
```

Text output renders them as:

```text
remediation-caused-resolution: NOT ESTABLISHED
recurrence-root-cause: NOT ESTABLISHED
git-causation: NOT ESTABLISHED
```

## CLI

Read-only replay:

```text
diagprint replay <HISTORY> <FINGERPRINT> [--format text|json]
diagprint history replay <HISTORY> <FINGERPRINT> [--format text|json]
```

Evidence verification:

```text
diagprint history remediation-verify <HISTORY>
```

The fingerprint may be exact or a unique leading hexadecimal prefix at the CLI
boundary.

Both replay commands verify the diagnostic history and remediation evidence
sidecars before presenting conclusions.

No replay command applies edits or executes commands.
