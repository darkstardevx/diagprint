# Plan: E1 error-stack Ecosystem Bridge + diagprint Bridge SDK

Status: Approved

## Plan revision

This Approved plan was expanded before any E1 implementation landed.

The first E1 implementation checkpoint now creates a reusable, ecosystem-neutral
interoperability SDK:

```text
crates/diagprint-bridge
package: diagprint-bridge
```

`diagprint-error-stack` becomes the first adapter implemented on top of that SDK.

This revision exists because the first adapter exposed a durable architectural
pattern worth owning directly:

```text
external structured diagnostic ecosystem
                │
                ▼
        adapter-specific mapper
                │
                ▼
       BridgeDiagnosticMetadata
                │
                ▼
         BridgeOutputBuilder
           │             │
           ▼             ▼
   DiagnosticReport   M4 relationship graph
```

The SDK belongs to diagprint. It does not adopt a third-party bridge framework.

It builds on diagprint's existing `InteropDiagnostic` protocol rather than
creating a competing normalized diagnostic representation.

## Product thesis

> Use the error and diagnostic tools you already like. Diagprint connects them
> into one structured diagnostic lifecycle.

E1 is the first implementation milestone in the Ecosystem Bridges track.

The reusable SDK should make later adapters smaller, more consistent, and safer.

Future adapters such as SNAFU, eyre/color-eyre, tracing-error, compiler/tooling,
or other structured producers should be able to reuse the same bridge mechanics
without depending on the error-stack adapter.

## Goals

E1 now has two related product goals:

1. establish `diagprint-bridge` as diagprint's reusable adapter SDK;
2. establish `diagprint-error-stack` as its first real ecosystem adapter.

The combined architecture must preserve:

- canonical logical identity;
- diagnostic instance multiplicity;
- M4 relationship semantics/evidence;
- source topology;
- adapter producer provenance;
- privacy boundaries;
- stable-only upstream APIs;
- root core independence from fast-moving external crates.

## Non-goals

E1 will not:

- create a universal Rust error trait;
- replace `std::error::Error`;
- replace `InteropDiagnostic`;
- make diagprint core depend on third-party error crates;
- invent cross-ecosystem causal truth;
- create persistent identity from addresses, `TypeId`, traversal order, or branch indexes;
- serialize bridge-construction node handles;
- build a plugin ABI;
- build proc macros for adapter generation;
- implement SNAFU/eyre/tracing-error in the same milestone;
- automatically export backtraces or span traces;
- require nightly provider APIs.

## Upstream research snapshot

Verified before E1 implementation:

```text
crate:        error-stack
version:      0.8.0
released:     2026-07-03
rust-version: 1.83.0
diagprint MSRV: 1.85.0
```

Stable structured APIs relevant to E1 include:

```text
Report<C: ?Sized>
Report::frames()
Report<C>::current_frame()
Report<[C]>::current_frames()
Frame::sources()
Frame::kind()
FrameKind::Context
FrameKind::Attachment
AttachmentKind::Printable
AttachmentKind::Opaque
Frame::is<T>()
Frame::downcast_ref<T>()
Report::contains<T>()
Report::downcast_ref<T>()
```

Nightly-only provider-style APIs such as `request_ref` and `request_value` are
outside the required stable contract.

## Packaging decision

Create two workspace companion crates:

```text
crates/diagprint-bridge
package: diagprint-bridge

crates/diagprint-error-stack
package: diagprint-error-stack
```

### diagprint-bridge

Depends on:

```text
diagprint
std
```

No third-party diagnostic ecosystem dependency belongs in `diagprint-bridge`.

The SDK is versioned with the diagprint workspace release line.

### diagprint-error-stack

Depends on:

```text
diagprint
diagprint-bridge
error-stack 0.8
```

`error-stack` remains completely outside the root `diagprint` dependency graph.

Proposed upstream dependency:

```toml
[dependencies.error-stack]
version = "0.8"
default-features = false
features = ["std"]
```

The adapter should not enable error-stack's default backtrace feature merely by
being installed.

## Existing core interoperability boundary

diagprint core already exposes:

```text
InteropDiagnostic
InteropDiagnosticSource
InteropDiagnosticSourceExt
DiagnosticRelationship
DiagnosticRelationshipGraph
DiagnosticRelationshipKind
DiagnosticRelationshipEvidence
IDENTITY_ATTRIBUTE
```

`diagprint-bridge` composes these APIs.

It must not duplicate their responsibilities.

### Responsibility split

```text
InteropDiagnostic
    normalized diagnostic payload

DiagnosticRelationshipGraph
    durable logical relationship model

diagprint-bridge
    adapter construction SDK:
    - mapper output metadata
    - ephemeral node handles
    - logical identity attachment
    - output assembly
    - relation assembly
    - duplicate/self-relation handling
    - shared bridge statistics
    - bridge errors

adapter crate
    ecosystem-specific traversal and semantics
```

## diagprint-bridge public architecture

Proposed foundational public types:

```text
BridgeDiagnosticMetadata
BridgeNodeId
BridgeOutput
BridgeOutputBuilder
BridgeBuildStats
BridgeError
```

Exact naming may be refined during implementation.

### BridgeDiagnosticMetadata

Wraps:

```text
InteropDiagnostic
optional application-owned logical identity
```

It provides reusable adapter metadata methods for:

```text
severity
code
help
notes
labels
cause
documentation
related diagnostics
explicit logical identity
```

The normalized payload continues to come from `InteropDiagnostic`.

The SDK should not fork or mirror the entire diagnostic data model.

### Logical identity

Optional mapper-owned identity is attached using diagprint's existing
`IDENTITY_ATTRIBUTE`.

SDK identity input must reject obviously invalid values rather than silently
accepting accidental empty/control-character identifiers.

Minimum proposed rules:

```text
non-empty
bounded length
no ASCII control characters
```

Do not over-constrain application namespace syntax in v1.

### BridgeNodeId

`BridgeNodeId` is an opaque in-process construction handle.

Properties:

```text
Copy
Eq
Hash/Ord if useful
not Serialize
not a canonical identity
not exposed in final graph JSON
not derived from source addresses
```

Adapters use it to relate diagnostic instances while constructing output.

Persistent graph endpoints are always canonical diagnostic fingerprints.

### BridgeOutputBuilder

The central reusable builder.

Conceptual API:

```text
let mut bridge = BridgeOutputBuilder::new(reporter, "error-stack");

let source = bridge.push(metadata_a)?;
let outer = bridge.push(metadata_b)?;

bridge.relate(
    source,
    outer,
    DiagnosticRelationshipKind::ContributesTo,
    DiagnosticRelationshipEvidence::SourceChain,
)?;

let output = bridge.finish()?;
```

Responsibilities:

- convert `BridgeDiagnosticMetadata` into real `Diagnostic` values;
- append every diagnostic instance to `DiagnosticReport`;
- compute each instance's canonical logical fingerprint;
- retain an internal `BridgeNodeId -> fingerprint` mapping;
- construct M4 relationships from node handles;
- let M4 validate relationship kinds/evidence/producer;
- build and verify the final deterministic relationship graph;
- preserve duplicate diagnostic instances;
- collapse logical self-relations when two instance handles resolve to one fingerprint;
- count collapsed self-relations rather than manufacturing fake identity.

### BridgeOutput

Contains:

```text
DiagnosticReport
DiagnosticRelationshipGraph
BridgeBuildStats
```

Exposes immutable accessors and `into_parts()`/equivalent ownership ergonomics.

### BridgeBuildStats

Generic counts only.

Candidate fields:

```text
diagnostic_instances
logical_nodes
relationships
collapsed_self_relationships
```

Adapter-specific counts such as error-stack attachment frames remain in the
adapter crate.

### BridgeError

Must cover:

```text
invalid node handle
invalid mapper identity
relationship construction failure
graph construction/verification failure
bridge invariant violation
```

Preserve source errors where applicable.

## Mapper strategy

The SDK owns mapper *output*, not every adapter's mapper input trait.

This distinction is important.

Each ecosystem exposes different structured source objects.

Therefore:

```text
diagprint-bridge:
    BridgeDiagnosticMetadata

diagprint-error-stack:
    ErrorStackContextView
    ErrorStackContextMapper

future diagprint-snafu:
    SnafuContextView
    SnafuContextMapper
```

Every adapter-specific mapper returns the same reusable
`BridgeDiagnosticMetadata`.

This gives consistency without forcing unrelated ecosystems into one awkward
input trait.

## Privacy architecture

The SDK must be safe by default but must not pretend every ecosystem has the
same privacy surface.

SDK-level guarantees:

- no node handle persistence;
- no pointer/address persistence;
- no `TypeId` persistence;
- no implicit debug rendering;
- no automatic environment/filesystem capture;
- no automatic backtrace/span-trace capture.

Adapter-specific content policy remains in adapters unless repeated use proves a
generic policy belongs in the SDK.

E1B may promote a simple reusable text inclusion policy into `diagprint-bridge`
only if it cleanly applies without error-stack terminology.

Do not put `ErrorStackAttachmentPolicy` in the SDK.

## Core compatibility boundary

Protected by default:

```text
src/
tests/                  root integration tests
root diagprint feature list
root diagprint dependencies
```

The root workspace member list may change.

If `diagprint-bridge` requires new public core behavior rather than existing
public APIs, stop and revise the plan before touching root core source.

## error-stack adapter architecture

The adapter converts one `error_stack::Report` into output built through
`BridgeOutputBuilder`.

Proposed public adapter types:

```text
ErrorStackBridge
ErrorStackBridgeConfig
ErrorStackBridgeOutput
ErrorStackReportExt
ErrorStackContextView
ErrorStackContextMapper
ErrorStackAttachmentPolicy
ErrorStackBridgeError
```

`ErrorStackContextMetadata` from the original plan is replaced by the reusable:

```text
diagprint_bridge::BridgeDiagnosticMetadata
```

## Context frames become diagnostics

Each `FrameKind::Context` becomes one diagnostic instance by pushing
`BridgeDiagnosticMetadata` into `BridgeOutputBuilder`.

Default mapping:

```text
severity: error
message: context Display value
code: none
explicit identity: none
```

Formatting the actual context object's `Display` is allowed.

Parsing the rendered `Report` is forbidden.

## Stable identity policy

The default mapper invents no external stable identity.

Typed application mappers may use stable downcasting to supply domain metadata
and `BridgeDiagnosticMetadata::identity(...)`.

Forbidden persistent identity inputs:

- frame address;
- pointer address;
- `TypeId`;
- traversal depth;
- frame index;
- grouped branch index;
- rendered debug text.

## Logical identity versus instances

`DiagnosticReport` preserves every diagnostic instance.

M4 graph nodes remain logical fingerprints.

`BridgeNodeId` allows adapters to represent instance topology while assembling
the graph.

If a relation connects two instance handles that map to one logical
fingerprint, `BridgeOutputBuilder` records a collapsed logical self-relation and
does not emit an invalid M4 self-edge.

This behavior is centralized in the SDK so every future adapter gets it
consistently.

## Source-chain relationship mapping

Use `Frame::sources()`.

Do not infer topology from flat `Report::frames()` order.

When one context frame is the structured source of another context frame:

```text
kind:     contributes_to
evidence: source_chain
producer: error-stack
```

Direction:

```text
source/deeper context  --contributes_to-->  outer/current context
```

The adapter should pass the relevant `BridgeNodeId`s to the SDK builder.

No default `causes` edge.

No inferred-correlation edge.

No Git provenance edge.

## Multiple current contexts

Support both:

```text
Report<C>
Report<[C]>
```

Single:

```text
Report::current_frame()
```

Grouped:

```text
Report::current_frames()
```

Sibling grouped branches are not related merely because they share one grouped
report.

Only upstream structure creates relations.

Ephemeral source-frame addresses may still be used inside the error-stack
adapter as a traversal visited/memoization key if needed.

Those addresses must never become `BridgeNodeId`, canonical identity, serialized
output, or graph data.

## Attachment policy

Attachments enrich diagnostics; they are not graph nodes.

Default:

```text
printable attachment content: omitted
opaque attachment content: omitted
```

E1B adds adapter-level:

```text
ErrorStackAttachmentPolicy::Omit
ErrorStackAttachmentPolicy::PrintableText
```

Printable inclusion is explicit opt-in.

Opaque attachment values remain omitted unless a future reviewed typed mapping
extension handles them.

Attachment frames may be traversed through to preserve source topology.

Attachments never create source-chain edges by themselves.

## No renderer archaeology

Forbidden:

```text
format!("{report:?}") then parse
format!("{report:#}") then parse
regex over pretty output
ANSI stripping to recover structure
box-drawing glyph parsing
```

Use stable structured APIs only.

## Workspace and release integration

By E1 completion, add both new packages to:

```text
[workspace].members
scripts/release-gates packages[]
scripts/release-gates satellite release list
.github/workflows/ci.yml package readiness list
```

Workspace default/all-feature/MSRV gates then exercise both automatically.

Package checks:

```text
cargo package --list -p diagprint-bridge
cargo package --list -p diagprint-error-stack
```

Satellite publish dry-runs include both.

## Expected implementation files

### E1A0 SDK foundation

New:

```text
crates/diagprint-bridge/Cargo.toml
crates/diagprint-bridge/README.md
crates/diagprint-bridge/src/lib.rs
crates/diagprint-bridge/tests/builder.rs
crates/diagprint-bridge/examples/custom_bridge.rs
```

Modified:

```text
Cargo.toml
Cargo.lock
```

### E1A error-stack adapter

New:

```text
crates/diagprint-error-stack/Cargo.toml
crates/diagprint-error-stack/README.md
crates/diagprint-error-stack/src/lib.rs
crates/diagprint-error-stack/tests/bridge.rs
crates/diagprint-error-stack/examples/basic.rs
```

Potential split modules:

```text
crates/diagprint-error-stack/src/bridge.rs
crates/diagprint-error-stack/src/mapper.rs
```

### E1B integration

Expected modified:

```text
.github/workflows/ci.yml
scripts/release-gates
README.md
CHANGELOG.md
docs/ecosystem-bridges-roadmap.md
.plans/E1-error-stack.plan.md
```

Optional E1B tests:

```text
crates/diagprint-error-stack/tests/grouped.rs
crates/diagprint-error-stack/tests/privacy.rs
```

Protected unless the plan is revised:

```text
src/
tests/
crates/diagprint-derive/
crates/diagprint-lsp/
crates/diagprint-async/
crates/diagprint-otel/
crates/diagprint-test/
```

## SDK test matrix

### Metadata normalization

Prove:

- `BridgeDiagnosticMetadata` uses `InteropDiagnostic`;
- code/severity/help/notes/labels/cause/documentation survive conversion;
- optional logical identity reaches `IDENTITY_ATTRIBUTE`;
- invalid identity fails closed.

### Node handles

Prove:

- every pushed diagnostic returns an opaque `BridgeNodeId`;
- node handles are instance construction references only;
- final output contains fingerprints, not node-handle values;
- duplicate logical diagnostics may have different node handles.

### Relationship assembly

Prove:

- relating two handles creates an M4 relationship with the chosen kind/evidence;
- producer passes through M4 validation;
- unknown node handles fail closed;
- M4 invalid relation/evidence combinations fail closed;
- deterministic graph identity remains intact.

### Logical self-collapse

Push two instances with one explicit logical identity.

Relate them.

Prove:

- report retains both instances;
- graph contains one logical node;
- no self-edge is emitted;
- collapsed-self count increments.

### No adapter dependency

Prove `diagprint-bridge` has no dependency on:

```text
error-stack
snafu
eyre
tracing-error
```

## error-stack test matrix

### Basic context conversion

Prove:

- `Report<C>` converts without parsing rendered output;
- every context frame becomes a diagnostic instance;
- M4 graph verifies.

### Source chain

For:

```text
root -> middle -> outer
```

Prove:

```text
root   --contributes_to/source_chain--> middle
middle --contributes_to/source_chain--> outer
```

No default `causes`.

No inferred edge.

### Typed mapper

Adapter-specific mapper returns `BridgeDiagnosticMetadata`.

Prove downcast-based mapping can supply:

- code;
- severity;
- help;
- notes;
- explicit identity.

### Grouped reports

`Report<[C]>`:

- every branch traversed;
- branch source chains preserved;
- siblings not linked without upstream evidence.

### Attachments/privacy

Default output excludes printable and opaque secret sentinels.

Printable opt-in exposes only the selected printable content.

Opaque values remain private.

### Duplicate logical diagnostics

Prove SDK behavior is reused rather than reimplemented in the adapter.

## Stable-only and MSRV contract

Build and test without:

```text
nightly Rust
error-stack/unstable
```

Rust 1.85 remains the workspace MSRV.

`error-stack` currently declares Rust 1.83.

## Implementation sequence

### E1A0 — diagprint-bridge SDK foundation

Add:

- new `diagprint-bridge` workspace crate;
- `BridgeDiagnosticMetadata`;
- `BridgeNodeId`;
- `BridgeOutput`;
- `BridgeOutputBuilder`;
- `BridgeBuildStats`;
- `BridgeError`;
- builder tests;
- SDK README;
- custom-adapter example.

No third-party ecosystem dependency.

Gate:

```bash
cargo test -p diagprint-bridge --all-targets
cargo clippy -p diagprint-bridge --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p diagprint-bridge --no-deps
./scripts/gate.sh fast
```

Commit and verify exact CI before E1A.

### E1A — error-stack stable frame conversion

Add:

- `diagprint-error-stack` workspace crate;
- dependency on `diagprint-bridge`;
- adapter-specific mapper input trait/view;
- mapper output through `BridgeDiagnosticMetadata`;
- single `Report<C>` conversion;
- source-chain relations through `BridgeOutputBuilder`;
- basic tests;
- README/example.

Gate:

```bash
cargo test -p diagprint-error-stack --all-targets
cargo clippy -p diagprint-error-stack --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p diagprint-error-stack --no-deps
./scripts/gate.sh fast
```

Commit and verify exact CI before E1B.

### E1B — grouped reports, privacy, release integration

Add:

- `Report<[C]>`;
- attachment privacy policy;
- privacy regression tests;
- grouped tests;
- CI package readiness for both new crates;
- release-gate registration for both new crates;
- root README/CHANGELOG;
- ecosystem roadmap updates;
- full workspace/MSRV validation.

Gate:

```bash
./scripts/gate.sh full
```

Commit and verify exact CI.

## Acceptance criteria

E1 is complete when:

- `diagprint-bridge` exists as a publishable reusable SDK;
- SDK depends on diagprint core but on no third-party diagnostic ecosystem;
- `BridgeDiagnosticMetadata` reuses `InteropDiagnostic`;
- `BridgeOutputBuilder` centralizes diagnostic/report/graph assembly;
- `BridgeNodeId` is ephemeral and never durable identity;
- duplicate logical instances are preserved in reports;
- logical self-relations collapse centrally and are counted;
- relationship semantics/evidence continue through M4 validation;
- no root diagprint dependency on error-stack exists;
- `diagprint-error-stack` exists as a publishable companion crate;
- `Report<C>` and `Report<[C]>` are supported;
- source topology comes from `Frame::sources()`;
- source edges use `ContributesTo + SourceChain`;
- no default `Causes` relationship is invented;
- attachment content is omitted by default;
- printable attachment text requires explicit opt-in;
- typed mappers can improve semantic metadata and logical identity;
- no renderer archaeology exists;
- both crates pass strict Clippy/rustdoc/tests;
- both crates are registered in package/release gates;
- Rust 1.85 remains green;
- exact CI succeeds at every implementation checkpoint.

## Future SDK growth

Do not overload v1.

After E1 and at least one additional adapter, evaluate promoting more repeated
patterns into `diagprint-bridge`, such as:

- generic content/privacy policy types;
- bridge conformance test helpers;
- source-topology utilities;
- adapter capability metadata;
- bridge version negotiation;
- adapter inventory/registration;
- richer safe attachment mapping;
- standard exporter capability descriptors.

Only extract patterns demonstrated by real adapters.

## Completion record

```text
E1A0 SDK commit:
E1A0 CI run:
E1A0 CI result:

E1A adapter commit:
E1A CI run:
E1A CI result:

Final E1B commit:
Final CI run:
Final CI result:

Notes:
```
