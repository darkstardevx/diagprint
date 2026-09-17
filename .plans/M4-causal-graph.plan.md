# Plan: M4 Causal Diagnostic Graph

Status: Approved

## Goal

Build the foundational typed relationship graph for diagprint Diagnostic
Forensics.

M4 turns isolated diagnostic fingerprints into a verifiable graph of
relationships while preserving the central forensic rule that evidence strength
and causal semantics are separate things.

The foundation must support:

- explicit producer-declared diagnostic relationships;
- structural/source-chain relationships;
- observed temporal associations;
- inferred correlations that are never promoted to causal facts;
- deterministic graph construction;
- privacy-light history binding;
- graph traversal and cascade analysis;
- text, JSON, and Graphviz DOT presentation;
- extension points for Ecosystem Bridges.

The user-facing thesis is:

> Use the error and diagnostic tools you already like. Diagprint connects them
> into one structured diagnostic lifecycle.

M4 is the relationship layer that makes that interoperability possible.

## Non-goals

M4 will not:

- claim automated root cause;
- infer that a Git commit caused a diagnostic;
- treat co-occurrence as causation;
- convert the existing free-form `Diagnostic::cause` message chain into stable
  cross-diagnostic causal edges;
- change canonical-v1 fingerprint or diagnostic-digest semantics;
- change history-v2 run files or chain semantics;
- change capsule-v1 semantics;
- execute remediation;
- determine whether a remediation succeeded;
- add a machine-learning model;
- add arbitrary numeric "confidence" scores;
- require Graphviz to be installed;
- add an Ecosystem Bridge implementation in the same Rust milestone.

Bridge implementations are an interleaved track after the graph foundation is
landed.

## Context

Existing diagnostic identity is already strong:

- `DiagnosticFingerprint` identifies one logical diagnostic;
- `DiagnosticDigest` identifies meaningful diagnostic content;
- `ReportDigest` identifies report content;
- all are defined under immutable `diagprint.canonical/v1`.

Existing history stores only privacy-light observations:

```text
fingerprint
digest
severity
```

History v2 is append-only and hash chained.

Existing forensic layers are:

```text
M1  diagprint why
M2  diagprint timeline
M3  diagprint blame / Git provenance
```

M3 deliberately models repository provenance as evidence without claiming
causation.

`Diagnostic` already contains a nested textual `Cause` chain. That structure is
human-readable error-source context, not a stable relationship between
canonical diagnostic identities. M4 must not overload it.

## Architecture placement

Introduce a new core library module:

```text
src/relationship.rs
```

This owns:

- relationship kinds;
- evidence classification;
- relationship records;
- graph nodes/edges;
- deterministic graph construction;
- validation;
- traversal;
- cycle-safe graph algorithms;
- serialization contracts.

Extend:

```text
src/forensics.rs
```

only for history-aware forensic projections over relationship evidence.

Add a sidecar persistence module only if the implementation remains clearer
than placing the sidecar code in `relationship.rs`.

CLI orchestration remains in:

```text
src/bin/diagprint.rs
```

Public exports are wired through:

```text
src/lib.rs
```

M4 should not modify `DiagnosticReport` storage merely to carry relationships.
The relationship graph is a separate evidence layer keyed by canonical
diagnostic identity.

This keeps canonical report identity unchanged and makes the same graph API
usable by native diagprint producers and external ecosystem adapters.

## Data flow

Native or bridge producer:

```text
diagnostics
    │
    ├── canonical fingerprints/digests
    │
    ▼
relationship declarations
    │
    ▼
DiagnosticRelationshipGraph
    │
    ├── validate endpoint identities
    ├── classify relation semantics
    ├── classify evidence provenance
    ├── deterministic ordering
    └── graph digest
```

When history is enabled:

```text
DiagnosticHistoryRun
    │
    ├── exact run digest
    ├── exact report digest
    │
    ▼
RelationshipSnapshot sidecar
    │
    ├── graph digest
    ├── endpoint fingerprints
    ├── typed edges
    └── evidence classes
```

Forensic projection:

```text
verified history
    +
verified relationship sidecars
    +
optional Git provenance
    │
    ▼
diagprint graph
    │
    ├── neighborhood
    ├── upstream/downstream traversal
    ├── explicit causal paths
    ├── structural paths
    ├── correlation-only paths
    └── lifecycle/provenance annotations
```

## Invariants

### Identity

Every graph node representing a diagnostic is keyed by the full qualified
canonical `DiagnosticFingerprint`.

Graph identity must not alter canonical-v1 identity.

Diagnostic content digests may annotate observations but do not replace logical
fingerprint identity.

### Causality discipline

Relationship semantics and evidence provenance are distinct.

A proposed shape is:

```text
RelationshipKind
  causes
  contributes_to
  depends_on
  derived_from
  precedes
  co_occurs_with
  related_to
```

and separately:

```text
RelationshipEvidence
  producer_declared
  source_chain
  structural
  trace_context
  temporal_association
  inferred_correlation
```

The final naming may be refined during implementation, but the separation is
mandatory.

`inferred_correlation` must never serialize or display as `causes`.

Temporal association must never serialize or display as causal proof.

Git provenance remains repository context only.

### Explicit versus inferred evidence

The graph API must make explicit producer relationships distinguishable from
relationships generated by an inference pass.

Inference may create correlation/association edges.

Inference must not synthesize `causes` edges.

### History

Do not modify:

```text
diagprint.history.run/v2
diagprint.history.head/v1
```

Relationship persistence must be a sidecar bound to an exact immutable history
run digest and report digest.

Tampering with a sidecar must be detectable.

A sidecar must not silently rebind to another history run.

### Privacy

Persistent graph evidence must remain privacy-light.

Do not persist:

- diagnostic message text;
- source text;
- source paths;
- arbitrary diagnostic attributes;
- help/notes;
- remediation payloads;
- Git author/email;
- repository remotes;
- absolute repository paths;
- trace field values unless a future schema explicitly permits them.

Persist only stable diagnostic identities, typed relationship metadata,
bounded producer identifiers, run/report bindings, and cryptographic digests.

### Determinism

Equivalent relationship input must produce byte-stable deterministic graph
identity regardless of insertion order.

Nodes and edges must have canonical deterministic ordering before hashing or
serialization.

Duplicate identical relationships should collapse deterministically.

Conflicting relationship declarations must remain representable rather than
being silently overwritten.

### Cycles

Do not assume the complete relationship graph is a DAG.

Dependency, correlation, and external-system relationships may contain cycles.

Traversal must be cycle-safe and bounded.

Any operation specifically requiring acyclicity must validate that requirement
rather than assuming it.

### Compatibility

No dependency is added unless implementation proves one is necessary.

Prefer std collections plus existing serde/serde_json/sha2 support.

Rust 1.85 remains the MSRV.

## Schema / persistence

Proposed new immutable schemas:

```text
diagprint.relationship.graph/v1
diagprint.relationship.snapshot/v1
```

Names may be refined before implementation, but once committed they become
versioned contracts.

### Graph

The graph representation should contain:

```text
schema
nodes
edges
graph_digest
```

A diagnostic node minimally contains:

```text
fingerprint
```

An edge minimally contains:

```text
from
to
kind
evidence
producer
```

Optional evidence details must remain structured, bounded, privacy-safe, and
deterministically ordered.

### Snapshot

A persisted history-bound snapshot should contain:

```text
schema
run_index
history_run_digest
report_digest
graph_digest
snapshot_digest
```

and enough graph content or exact graph reference data for independent
verification.

Sidecars should use create-new append-only persistence, with identical writes
idempotent and conflicting replacements rejected.

Proposed location:

```text
<HISTORY>/relationships/run-XXXXXX.json
```

This mirrors the successful Git-provenance sidecar architecture without
changing history-v2.

## Public API / CLI

Proposed library surface:

```text
DiagnosticRelationshipKind
DiagnosticRelationshipEvidence
DiagnosticRelationship
DiagnosticRelationshipGraph
DiagnosticRelationshipGraphBuilder
DiagnosticRelationshipSnapshot
DiagnosticRelationshipError
```

Likely graph operations:

```text
nodes()
edges()
neighbors()
incoming()
outgoing()
ancestors()
descendants()
paths()
subgraph()
roots_for_explicit_causal_edges()
strongly_connected_components()   [only if justified]
```

Do not name an algorithm result `root_cause`.

Prefer terms such as:

```text
upstream explicit source
explicit causal predecessor
cascade origin within recorded explicit edges
```

These describe graph structure without claiming truth beyond the recorded
evidence.

### CLI

Add:

```text
diagprint graph <HISTORY> <FINGERPRINT>
```

Useful options should include:

```text
--run <INDEX>
--depth <N>
--direction upstream|downstream|both
--evidence explicit|all
--format text|json|dot
```

Also support:

```text
diagprint history graph ...
```

as an alias if consistent with existing history subcommands.

CLI fingerprint selection may use the existing exact-or-unique-prefix resolver.

The library API continues to require full canonical identities.

### Text presentation

The terminal view must visually separate:

```text
EXPLICIT RELATIONSHIPS
STRUCTURAL / SOURCE RELATIONSHIPS
ASSOCIATIONS
INFERRED CORRELATIONS
```

and must not collapse them into one unlabeled arrow type.

When Git provenance is shown:

```text
repository association: verified
causation from Git: NOT ESTABLISHED
```

### DOT

DOT output must:

- require no Graphviz dependency;
- emit valid deterministic DOT text;
- encode relation/evidence labels;
- remain useful to external visualization tools.

## Ecosystem Bridge extension contract

M4 must make third-party adapters able to produce graph evidence without
depending on CLI internals.

A bridge should be able to:

1. convert upstream structured errors/diagnostics into diagprint diagnostics;
2. retain stable upstream identities where available;
3. emit typed diagnostic relationships;
4. describe relationship evidence provenance;
5. avoid flattening rich structure to `to_string()` when structured access is
   available.

M4 does not implement the first bridge, but its public API must make the first
bridge possible without redesign.

## Ecosystem Bridges track

The bridge train is interleaved with M4/M5 rather than developed as unreviewed
parallel Rust changes.

Initial order:

```text
M4  causal diagnostic graph foundation
E1  error-stack bridge
M5  replay/regression/remediation evidence
E2  SNAFU bridge
E3  eyre/color-eyre bridge
E4  tracing-error / span-context bridge
```

Future candidates are evaluated for:

- active maintenance;
- ecosystem relevance;
- structured information exposed by public API;
- MSRV compatibility;
- dependency weight;
- whether integration belongs in core or a companion crate.

Large or fast-moving adapters should prefer companion crates.

## Privacy analysis

M4 persists diagnostic identity and typed relationship evidence, not diagnostic
payload text.

Producer identifiers must be low-cardinality integration identifiers such as:

```text
diagprint.native
error-stack
snafu
tracing-error
rustc
cargo
```

They must not become arbitrary application logging fields.

Inference implementations must not persist source text merely to explain why an
edge exists.

If richer evidence is useful at presentation time, query it from the original
system when available instead of widening the durable privacy surface.

## Compatibility analysis

### Canonical identity

No changes to canonical-v1 fingerprint, diagnostic digest, or report digest
construction.

### Diagnostic serialization

Do not add relationship fields directly to `Diagnostic` if doing so would
silently alter existing serialization or canonical content behavior.

### History

No changes to history-v2 run schema or hash-chain semantics.

### Git provenance

M3 provenance sidecars remain byte- and semantically compatible.

Relationship graph may annotate a view with Git provenance, but Git data does
not become causal graph truth.

### Capsules

Capsule-v1 implementation remains unchanged in the initial M4 foundation unless
a later M4 substep explicitly defines a backward-compatible project-provenance
anchor. If anchoring cannot be added without changing capsule-v1 semantics,
defer it to a new capsule version.

### Features

No new feature flag for the core graph unless compilation/dependency analysis
proves it useful.

Ecosystem adapters may use optional features or companion crates according to
dependency weight and stability.

### MSRV

All M4 code must compile on Rust 1.85.

## Expected file boundary

Expected M4 implementation files:

```text
src/relationship.rs
src/forensics.rs
src/lib.rs
src/bin/diagprint.rs
tests/relationship_graph.rs
tests/relationship_graph_cli.rs
docs/forensics-relationship-graph-v1.md
README.md
CHANGELOG.md
.plans/M4-causal-graph.plan.md
```

Potential additional file only if justified by implementation structure:

```text
src/relationship_store.rs
```

Protected by default:

```text
Cargo.toml
Cargo.lock
src/canonical.rs
src/fingerprint.rs
src/history.rs
src/capsule.rs
crates/
```

Changes to protected files require stopping and revising this plan before
continuing.

## Test-first matrix

### Identity and determinism

Prove:

- insertion order does not change graph digest;
- node ordering is deterministic;
- edge ordering is deterministic;
- duplicate identical edges collapse consistently;
- relation kinds remain distinct;
- evidence kinds remain distinct.

### Validation

Reject:

- malformed fingerprints;
- self-relations when forbidden by relation kind;
- impossible snapshot run bindings;
- report-digest mismatches;
- history-run-digest mismatches;
- snapshot digest tampering;
- graph digest tampering.

If self-relations are allowed for any relation kind, test and document why.

### Causality discipline

Prove:

- inferred correlation never becomes `causes`;
- temporal association never becomes `causes`;
- Git provenance never becomes `causes`;
- text output labels inferred edges explicitly;
- JSON preserves evidence class;
- DOT preserves relation/evidence class.

### Graph algorithms

Prove:

- upstream traversal;
- downstream traversal;
- bounded depth;
- cycle safety;
- deterministic paths/subgraphs;
- multiple incoming/outgoing edges;
- disconnected diagnostics;
- graph selection by exact fingerprint;
- CLI unique-prefix behavior.

### Persistence

Prove:

- snapshot round-trip;
- exact history-run binding;
- exact report binding;
- append-only create-new persistence;
- idempotent identical persistence;
- conflicting replacement rejection;
- tamper detection.

### Privacy

Prove serialized relationship snapshots do not contain:

- diagnostic message text;
- source paths;
- source text;
- help/notes;
- arbitrary attributes;
- remediation contents;
- Git author/email/remote/path.

### Existing forensics

Retain green coverage for:

```text
diagnostic_forensics
history_cli
git_provenance
git_blame_cli
```

### Feature/MSRV

Run all existing fast/full gates and Rust 1.85 validation.

## Implementation sequence

### Step 1 — Relationship model

Add typed node/edge/evidence structures and validation.

Gate:

```bash
cargo test --test relationship_graph --no-default-features
cargo clippy --lib --no-default-features -- -D warnings
```

### Step 2 — Deterministic graph identity

Add canonical deterministic graph/snapshot digest payloads without modifying
canonical-v1 diagnostic identity.

Gate:

```bash
cargo test --test relationship_graph
```

### Step 3 — History-bound sidecar

Add append-only relationship snapshot persistence bound to exact history and
report digests.

Do not alter history-v2 files.

Gate:

```bash
cargo test --test relationship_graph
```

### Step 4 — Forensic traversal

Add history-aware graph projection and cycle-safe upstream/downstream cascade
analysis.

Do not introduce root-cause claims.

Gate:

```bash
cargo test --test relationship_graph
./scripts/gate.sh forensics
```

### Step 5 — CLI

Add `diagprint graph` and history alias, deterministic text/JSON/DOT output,
depth and evidence filters.

Gate:

```bash
cargo test --test relationship_graph_cli
```

### Step 6 — Documentation

Document schema, privacy, relation semantics, evidence semantics, graph digest,
history binding, DOT output, and the causality boundary.

Update README roadmap and Ecosystem Bridges thesis.

### Step 7 — Full validation

Run:

```bash
./scripts/gate.sh precommit
./scripts/gate.sh fast
./scripts/gate.sh forensics
./scripts/gate.sh full
```

Then commit and verify CI on the exact SHA.

## Failure modes

### Accidental canonical-v1 change

Stop immediately.

Do not modify canonical identity to "make relationships fit."

Keep relationships in their own evidence layer.

### History schema pressure

If implementation appears to require relationship fields in history-v2, stop
and redesign around the sidecar.

### Free-form cause confusion

Do not hash or parse `Diagnostic::cause` text into cross-diagnostic causal
identity.

A future adapter may classify a source chain as structured evidence when it can
do so without guessing.

### Inference accidentally labeled causal

Treat as a correctness bug.

Inference may create association/correlation edges only.

### Cyclic graph

Do not crash or recurse indefinitely.

Traversal must use visited sets and explicit depth bounds.

### Partial script failure

Inspect the partial tree and resume from the failed stage.

Never reset or clean blindly.

## Acceptance criteria

M4 is complete when:

- typed diagnostic relationships exist;
- semantics and evidence provenance are separate;
- explicit and inferred relationships cannot be confused;
- deterministic graph identity exists;
- history-bound relationship evidence is tamper-evident;
- history-v2 remains unchanged;
- canonical-v1 remains unchanged;
- graph traversal is cycle-safe;
- `diagprint graph` provides text, JSON, and DOT views;
- Git association remains explicitly non-causal;
- privacy-light persistence is tested;
- all previous forensic milestones remain green;
- Rust 1.85 remains green;
- no unnecessary dependency is added;
- full local gates are green;
- CI succeeds on the exact M4 implementation commit.

## Completion record

```text
Commit:
CI run:
CI result:
Notes:
```
