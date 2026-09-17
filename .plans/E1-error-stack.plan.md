# Plan: E1 error-stack Ecosystem Bridge

Status: Approved

## Product thesis

> Use the error and diagnostic tools you already like. Diagprint connects them
> into one structured diagnostic lifecycle.

E1 is the first implementation milestone in the Ecosystem Bridges track.

The bridge must let applications keep using `error-stack` as their error
construction and propagation model while gaining diagprint lifecycle features:

- canonical diagnostic identity;
- diagnostic reports;
- M4 relationship graphs;
- history;
- forensic `why` and timeline analysis;
- relationship graph inspection;
- Git provenance;
- future M5 replay/regression/remediation evidence;
- export and telemetry surfaces.

The integration must preserve upstream structure rather than parsing
`error-stack`'s rendered `Display` or `Debug` output.

## Upstream research snapshot

Verified before this plan was opened:

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

`error-stack` supports both:

```text
Report<C>    one current context
Report<[C]> one or more current contexts / grouped errors
```

Nightly-only provider-style APIs such as frame/report `request_ref` and
`request_value` are outside E1's required stable contract.

`error-stack` 0.8 also recently changed the mutable frame traversal API.
That upstream evolution is a major reason E1 belongs in an isolated companion
crate rather than adding another fast-moving dependency to diagprint core.

## Packaging decision

Create a new workspace companion crate:

```text
crates/diagprint-error-stack
package: diagprint-error-stack
```

Do not add `error-stack` as a dependency of the root `diagprint` package.

The companion crate owns all direct knowledge of `error-stack`.

Proposed dependency shape:

```toml
[dependencies.diagprint]
version = "0.7.0"
path = "../.."

[dependencies.error-stack]
version = "0.8"
default-features = false
features = ["std"]
```

The exact manifest is validated during implementation, but the intent is to
avoid enabling `error-stack`'s default `backtrace` feature merely because the
bridge exists.

The application may independently enable other `error-stack` features.

## Core compatibility boundary

E1 should consume the M4 public API as it exists.

Protected by default:

```text
src/relationship.rs
src/history.rs
src/fingerprint.rs
src/canonical.rs
src/capsule.rs
src/bin/diagprint.rs
root diagprint feature list
root diagprint dependencies
```

If E1 cannot be implemented cleanly using the existing public graph/report
surface, stop and revise this plan before changing core.

Do not silently widen core just to make the first bridge easier.

## Architecture

The bridge converts one `error_stack::Report` into a bundle containing:

```text
DiagnosticReport
DiagnosticRelationshipGraph
bridge metadata needed for inspection/testing
```

Proposed public types:

```text
ErrorStackBridge
ErrorStackBridgeConfig
ErrorStackBridgeOutput
ErrorStackReportExt
ErrorStackContextMapper
ErrorStackContextMetadata
ErrorStackAttachmentPolicy
ErrorStackBridgeError
```

Exact names may be refined during implementation while preserving the
responsibilities below.

## Context frames become diagnostics

Each `FrameKind::Context` becomes one diagprint diagnostic instance.

The default mapper obtains the diagnostic message from the actual structured
context object through its `Display` implementation.

That is acceptable because the bridge is formatting the context object it was
given; it is not parsing a rendered `Report`.

The default mapping should use:

```text
severity: error
message: context Display value
code: none unless supplied by a mapper
stable explicit identity: none unless supplied by a mapper
```

This means the default bridge is useful for arbitrary `error-stack` reports,
while applications with typed domain errors can provide a mapper for stronger
metadata and identity.

## Stable identity policy

`error-stack` does not expose a generic portable string identity for every
erased `dyn Error` context frame.

E1 must not invent a persistent identity from:

- memory addresses;
- `TypeId`;
- frame iteration position;
- branch index;
- debug formatting;
- unstable implementation details.

Instead, expose `ErrorStackContextMapper`.

The mapper must be able to inspect the stable frame/context and, when it knows a
concrete application error type, use stable downcasting to provide:

```text
diagnostic code
severity
help
notes
explicit diagprint identity
other safe semantic metadata
```

A mapper can use `Frame::downcast_ref<T>()` for known types.

The default mapper leaves explicit identity unset and lets normal diagprint
canonical identity rules apply.

## Logical identity versus error instances

M4 is a logical diagnostic relationship graph.

If two `error-stack` branches map to the same diagprint logical fingerprint,
the graph may contain one logical node while `DiagnosticReport` still preserves
duplicate diagnostic instances.

Do not manufacture frame-instance IDs just to prevent logical deduplication.

If future use cases need an instance graph, that is a separate schema and plan.

## Source-chain relationship mapping

Use `Frame::sources()` to preserve actual upstream frame topology.

Do not infer topology from the flat `Report::frames()` order.

Walk from each current frame through its source frames.

Attachments may appear between context frames and should be traversed through,
not converted into fake diagnostic nodes.

When one context frame is the structured source of another context frame, emit:

```text
kind:     contributes_to
evidence: source_chain
producer: error-stack
```

Direction:

```text
source/deeper context  --contributes_to-->  outer/current context
```

E1 should not emit `causes` by default.

`ContributesTo` accurately preserves structured source-chain causality without
claiming that the deeper error is the sole cause.

No inferred-correlation edge is produced by this bridge.

No Git provenance edge is produced by this bridge.

## Multiple current contexts

Support both:

```text
Report<C>
Report<[C]>
```

For a single report, begin traversal at `current_frame()`.

For a grouped report, begin independently at every frame in `current_frames()`.

Do not create relationships between sibling current branches merely because
they appear in one `Report<[C]>`.

Preserve only relationships present in each branch's upstream frame topology.

The implementation must be safe if frame topology shares descendants or if a
future upstream version exposes a more graph-like structure.

Traversal may use ephemeral in-process frame addresses for a visited set only
if necessary to avoid duplicate traversal.

Ephemeral addresses must never be serialized, hashed, exposed as identity, or
persisted.

## Attachment policy

Attachments enrich diagnostics; they are not diagnostic nodes by default.

E1 must treat attachment content as potentially sensitive.

Default policy:

```text
attachment content: omitted
opaque attachment values: omitted
printable attachment values: omitted
```

The bridge may retain non-content counts/classification in its own output
metadata when doing so does not affect canonical logical identity.

Provide an explicit opt-in policy for printable attachment text.

Proposed policy:

```text
ErrorStackAttachmentPolicy::Omit
ErrorStackAttachmentPolicy::PrintableText
```

When `PrintableText` is selected, call the structured printable attachment's
`Display` implementation and attach the resulting text through an appropriate
diagprint note/enrichment surface.

Do not parse the complete `Report` rendering.

Opaque attachments remain omitted unless an application explicitly provides a
typed attachment mapper in a future or reviewed E1 extension.

E1 must not require nightly provider APIs to inspect arbitrary attachments.

## Attachment ownership

While walking one frame branch, printable/opaque attachment frames encountered
above a context belong to the nearest context reached below them in that
branch.

The bridge should collect pending attachment metadata while traversing
attachment frames and apply it when the owning context frame is reached.

Attachments must not cause source-chain edges by themselves.

## Privacy rules

Default bridge output must not copy:

- opaque attachment values;
- backtraces;
- span traces;
- arbitrary provided values;
- memory addresses;
- debug renderings of the full report;
- environment data;
- filesystem content;
- source text;
- Git information.

Printable attachment text is opt-in.

Custom context mappers are explicitly application-controlled and may enrich
diagnostics with application data.

Documentation must state that mapper-provided metadata follows the user's own
privacy policy.

## Backtrace and tracing policy

E1 does not automatically export `Backtrace` or `SpanTrace`.

The companion dependency should avoid enabling upstream default backtrace
capture on its own.

If the consuming application already uses those features, E1 still leaves those
objects untouched unless a later explicit bridge extension defines a safe
mapping.

This avoids large, path-heavy, privacy-sensitive diagnostic payloads.

## No renderer archaeology

Forbidden implementation strategies:

```text
format!("{report:?}") then parse lines
format!("{report:#}") then parse cause text
regex over error-stack terminal output
ANSI stripping to recover structure
counting box-drawing glyphs
```

Use only structured frame/report APIs.

A regression test should make this philosophy observable where practical.

## Public conversion surface

Desired ergonomics:

```text
report.to_diagprint(&reporter)
report.to_diagprint_with(&reporter, &mapper)
```

or equivalently an explicit bridge object:

```text
ErrorStackBridge::new(...)
    .convert(&report, &reporter)
```

Support must exist for both `Report<C>` and `Report<[C]>`.

The bridge output should expose immutable accessors for the resulting
`DiagnosticReport` and `DiagnosticRelationshipGraph`.

If ownership ergonomics are useful, also provide `into_parts()`.

## Mapper design

`ErrorStackContextMapper` should receive enough stable structured information
for typed applications to enrich a context without exposing bridge internals.

A likely input includes:

```text
&error_stack::Frame
&(dyn Error + Send + Sync + 'static)
branch/depth structural position for presentation only
```

Structural position is not identity.

Mapper output may include:

```text
severity
code
help
notes
explicit logical identity
```

Prefer a typed context object over a function with many positional arguments.

Do not suppress Clippy `too_many_arguments`; use structural parameter objects.

## Error behavior

The bridge should fail closed for invalid mapping output.

Examples:

- invalid explicit identity supplied by mapper;
- internal relationship endpoint missing;
- graph construction failure;
- unsupported invariant violation;
- mapper tries to create inconsistent context metadata.

Bridge errors should preserve useful source errors where applicable.

Do not panic on unusual but valid frame stacks.

## Workspace and release integration

Implementation must add `diagprint-error-stack` to:

```text
[workspace].members
scripts/release-gates packages[]
scripts/release-gates satellite release list
.github/workflows/ci.yml package readiness list
```

Workspace-wide default/all-feature/MSRV jobs will then exercise the crate
automatically.

No root feature-matrix entry is required because E1 is not a root feature.

Package readiness must include:

```text
cargo package --list -p diagprint-error-stack
```

Satellite release dry-runs must include the new package.

## Expected implementation files

New:

```text
crates/diagprint-error-stack/Cargo.toml
crates/diagprint-error-stack/README.md
crates/diagprint-error-stack/src/lib.rs
crates/diagprint-error-stack/tests/bridge.rs
crates/diagprint-error-stack/examples/basic.rs
```

Expected modified:

```text
Cargo.toml
Cargo.lock
.github/workflows/ci.yml
scripts/release-gates
README.md
CHANGELOG.md
docs/ecosystem-bridges-roadmap.md
.plans/E1-error-stack.plan.md
```

Optional if justified:

```text
crates/diagprint-error-stack/src/bridge.rs
crates/diagprint-error-stack/src/mapper.rs
crates/diagprint-error-stack/tests/grouped.rs
crates/diagprint-error-stack/tests/privacy.rs
```

Protected unless the plan is revised:

```text
src/
tests/              # root integration tests
crates/diagprint-derive/
crates/diagprint-lsp/
crates/diagprint-async/
crates/diagprint-otel/
crates/diagprint-test/
```

## Test matrix

### Basic context conversion

Prove:

- `Report<C>` converts without parsing rendered report output;
- every structured context frame becomes a diagnostic instance;
- the outer/current context is represented;
- deeper contexts are represented;
- resulting graph verifies successfully.

### Source-chain graph

Build:

```text
root context
  -> change_context middle
  -> change_context outer
```

Prove graph edges are:

```text
root   --contributes_to/source_chain--> middle
middle --contributes_to/source_chain--> outer
```

Prove no default `causes` edge exists.

Prove no inferred edge exists.

### Attachments

Test both printable and opaque attachments.

Default policy:

- attachment values are absent from diagnostics and bridge serialization/output;
- source-chain relationships still cross attachment frames correctly.

Printable opt-in:

- printable attachment `Display` text can be retained;
- opaque values remain absent.

Use obvious secret sentinel strings in privacy tests.

### Typed mapper

Create custom error types and mapper logic using stable downcasts.

Prove mapper can provide:

- code;
- severity;
- help;
- notes;
- explicit identity.

Prove mapper identity is stable across reports where volatile message values
change but the application-supplied logical identity remains constant.

### Grouped reports

Create `Report<[C]>` with multiple current branches.

Prove:

- every branch is traversed;
- each branch's source chain is preserved;
- sibling branches are not connected to each other;
- no fake common-cause node is introduced.

### Duplicate logical diagnostics

Create multiple error instances that intentionally map to one logical
fingerprint.

Prove:

- `DiagnosticReport` retains instances;
- M4 graph remains a logical graph and may deduplicate the node;
- no per-frame synthetic identity is introduced.

### Privacy

Default output must not contain sentinel values from:

- opaque attachment;
- printable attachment;
- backtrace-like test text;
- arbitrary debug-only attachment representation.

Printable opt-in should expose only the printable value intentionally selected.

### Stable-only contract

Build and test without nightly Rust or upstream unstable APIs.

Do not enable `error-stack/unstable`.

### MSRV

Run the complete workspace under Rust 1.85.

`error-stack` currently declares Rust 1.83, so the bridge should fit the
workspace MSRV if dependency resolution remains compatible.

### Package hygiene

Verify:

- root `diagprint` package does not gain an `error-stack` dependency;
- `diagprint-error-stack` package contents are intentional;
- satellite publish dry-run includes the bridge;
- the new crate README/doc examples compile.

## Implementation sequence

### E1A — companion crate skeleton and stable frame conversion

Add:

- workspace package;
- minimal dependencies;
- bridge config;
- context mapper abstraction;
- `Report<C>` support;
- diagnostic report output;
- source-chain relationship graph output;
- basic tests;
- README example.

Gate:

```bash
cargo test -p diagprint-error-stack
cargo clippy -p diagprint-error-stack --all-targets -- -D warnings
```

### E1B — grouped reports, attachment privacy, release integration

Add:

- `Report<[C]>` support;
- attachment policy;
- privacy tests;
- typed mapper tests;
- grouped-branch tests;
- CI/package/release-gate registration;
- root README and CHANGELOG docs.

Gate:

```bash
cargo test -p diagprint-error-stack --all-targets
./scripts/gate.sh fast
./scripts/gate.sh full
```

### Exact CI checkpoint

After implementation is committed:

```text
push exact E1 commit
verify GitHub CI success on that SHA
record commit/run in this plan
close E1 in a separate planning-only commit
```

## Acceptance criteria

E1 is complete when:

- `diagprint-error-stack` exists as a publishable companion crate;
- root diagprint has no direct `error-stack` dependency;
- stable `Report<C>` conversion works;
- stable grouped `Report<[C]>` conversion works;
- context frames become diagprint diagnostic instances;
- source topology comes from `Frame::sources()`;
- source-chain edges use `ContributesTo + SourceChain`;
- no default `Causes` edges are invented;
- sibling grouped branches are not linked without upstream evidence;
- attachment contents are omitted by default;
- printable attachment text requires explicit opt-in;
- opaque attachment values remain private by default;
- custom typed mappers can improve code/severity/help/notes/identity;
- no persistent identity uses addresses, TypeId, branch number, or frame order;
- no report-rendering parser exists;
- M4 graph verification remains green;
- workspace strict Clippy remains warning-free;
- workspace full tests/docs remain green;
- Rust 1.85 MSRV remains green;
- package readiness includes the new crate;
- satellite release gates include the new crate;
- exact implementation CI succeeds.

## Completion record

```text
E1A commit:
E1A CI run:
E1A CI result:

Final implementation commit:
Final CI run:
Final CI result:

Notes:
```
