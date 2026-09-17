# Plan: E2 SNAFU Ecosystem Bridge

Status: Approved

## Goal

Build `diagprint-snafu` as a first-class SNAFU interoperability crate whose
primary user experience is **typed extension traits** over the same flow shapes
SNAFU users already rely on:

```text
Result
Option
TryFuture
TryStream
```

Custom `#[derive(Snafu)]` error types are the primary E2 design target.

`Whatever` remains a first-class path, but E2 must not become a
"Whatever-only" adapter. The strongest outcome is that applications can keep
their domain-specific SNAFU enums/structs, preserve variant-specific typed
context, add diagprint identity/classification, and carry that structure through
sync and async control flow without flattening it to strings.

The E2 product rule is:

> SNAFU creates and enriches the error. diagprint-snafu preserves that typed
> error while attaching a structured diagnostic lifecycle.

## Baseline

E2 begins only after v0.8.0 fully released and closed.

```text
v0.8.0 published/tagged source:
9ab4252e7962c88ec6ff725889d9f4e66fe88bca

v0.8 closure commit:
634dee04dc74ab31e359a232dc6cf00d4b65cc52

v0.8 closure CI:
35273289319
success
```

E2 branches from the closure commit.

## Upstream research snapshot

```text
snafu:          0.9.2
released:       2026-07-21
SNAFU MSRV:     1.65
diagprint MSRV: 1.85
```

SNAFU's stable ergonomics include:

```text
ResultExt
OptionExt

with feature "futures":
  snafu::futures::TryFutureExt
  snafu::futures::TryStreamExt
```

SNAFU 0.9 changed `with_context` for Result/Future/Stream so the lazy closure
receives the underlying error.

SNAFU's async context combinators construct implicit location data when the
future/stream is polled, not necessarily where the combinator was created.
E2 must preserve/document this upstream semantic instead of implying otherwise.

Relevant stable SNAFU surfaces:

```text
std/core Error::source
snafu::ErrorCompat
ErrorCompat::backtrace
ErrorCompat::iter_chain
snafu::IntoError
snafu::FromString
snafu::NoneError
snafu::Whatever
snafu::WhateverLocal
snafu::ResultExt
snafu::OptionExt
snafu::futures::TryFutureExt
snafu::futures::TryStreamExt
```

Do not depend on SNAFU's unstable provider API.

## Packaging

Create:

```text
crates/diagprint-snafu
package: diagprint-snafu
```

Base dependencies:

```toml
[dependencies.diagprint]
version = "0.8.0"
path = "../.."

[dependencies.diagprint-bridge]
version = "0.8.0"
path = "../diagprint-bridge"

[dependencies.snafu]
version = "0.9.2"
default-features = false
features = ["std"]
```

Async support is opt-in:

```toml
[features]
default = []
futures = [
    "snafu/futures",
    "dep:futures",
]
```

Proposed optional dependency:

```toml
[dependencies.futures]
version = "0.3"
optional = true
default-features = false
features = ["std"]
```

Exact futures dependency shape may be minimized during implementation if
`futures-core` / `futures-util` are sufficient. Any change must keep the public
feature name `futures` and avoid a Tokio dependency.

SNAFU must remain outside root `diagprint` and `diagprint-bridge`.

Before future publication, explicitly verify availability/ownership of:

```text
diagprint-snafu
```

## Architecture

```text
SNAFU error / context selector
          |
          v
custom typed diagnostic metadata
          |
          v
SnafuBridge
          |
          v
diagprint-bridge
   |              |
   v              v
DiagnosticReport  M4 graph
```

The extension traits sit above this conversion layer:

```text
Result / Option / TryFuture / TryStream
          |
          v
diagprint-snafu extension trait
          |
          +--> SNAFU context construction when requested
          |
          +--> SnafuBridge conversion
          |
          v
CapturedSnafuError<E>
```

E2 must reuse `diagprint-bridge`; it does not create another graph/report
construction framework.

## Custom error types are first-class

A custom SNAFU enum or struct should be able to opt into rich diagprint
semantics directly.

Example application type:

```rust
#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Could not read config {path:?}"))]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },

    #[snafu(display("Invalid port {value}"))]
    InvalidPort {
        value: u16,
    },
}
```

E2 adds a diagprint-side customization trait.

Proposed:

```rust
pub trait SnafuDiagnostic:
    std::error::Error + snafu::ErrorCompat + 'static
{
    fn diagprint_metadata(
        &self,
    ) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError>;
}
```

This trait is intentionally application-controlled.

For an enum, implementation should normally match variants explicitly:

```rust
impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(
        &self,
    ) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        match self {
            Self::ReadConfig { path, .. } => {
                SnafuDiagnosticMetadata::new(
                    SnafuIdentity::new("config.read")?,
                    "Could not read configuration",
                )?
                .code(SnafuCode::new("CONFIG_READ")?)
                .severity(Severity::Error)
                .note(format!("config path: {}", path.display()))
                .finish()
            }

            Self::InvalidPort { value } => {
                SnafuDiagnosticMetadata::new(
                    SnafuIdentity::new("config.port.invalid")?,
                    "Invalid port",
                )?
                .code(SnafuCode::new("CONFIG_PORT")?)
                .note(format!("port: {value}"))
                .finish()
            }
        }
    }
}
```

Exact builder syntax may be refined, but variant-by-variant typed customization
is a required use case.

No derive macro for `SnafuDiagnostic` is required in E2.

A future proc-macro helper is allowed as a separate milestone only after the
manual trait contract proves stable.

## SnafuDiagnosticMetadata

Add a SNAFU-specific typed builder above `BridgeDiagnosticMetadata`.

Proposed:

```rust
pub struct SnafuDiagnosticMetadata {
    // private
}
```

Required root fields:

```text
SnafuIdentity
presentation message
```

Optional structured fields:

```text
SnafuCode
Severity
help
notes
labels
documentation
application-owned attributes where core supports them safely
```

The builder converts to `BridgeDiagnosticMetadata` only after validation.

This keeps stable classification types (`SnafuIdentity`, `SnafuCode`) separate
from arbitrary display strings.

### SnafuIdentity

```rust
pub struct SnafuIdentity(String);
```

Proposed validation:

```text
1..=128 bytes
first byte ASCII alphanumeric
remaining ASCII alphanumeric or . _ - :
no whitespace
no controls
```

Bridge form:

```text
snafu:<application-symbol>
```

Examples:

```text
config.read
config.port.invalid
database.connect
http.response.invalid
```

### SnafuCode

Canonical-v1 retains diagnostic code even when explicit identity is present.

Therefore E2 uses a strong code type instead of accepting arbitrary runtime
strings through the strong APIs:

```rust
pub struct SnafuCode(String);
```

Proposed validation:

```text
1..=64 bytes
first byte ASCII alphanumeric
remaining ASCII alphanumeric or . _ - :
no whitespace
no controls
```

Codes are optional but, when present, intentionally participate in logical
fingerprinting.

Never put paths, request IDs, user IDs, hostnames, free-form error text, or
other runtime values into `SnafuCode`.

## Canonical-v1 identity rule

Existing diagprint behavior:

```text
explicit diagprint.identity present:
  message does not participate in fingerprint
  primary source-label wording/location does not participate
  diagnostic code DOES participate
```

Required E2 guarantee:

```text
same SnafuIdentity
+ same SnafuCode (or both absent)
+ different dynamic presentation
= same logical fingerprint
```

The plan does not claim identity overrides code.

## CapturedSnafuError<E>

The extension traits must not throw away the application's original typed
error.

Add an owning capture type:

```rust
pub struct CapturedSnafuError<E> {
    error: E,
    capture: Result<SnafuBridgeOutput, SnafuBridgeError>,
}
```

The wrapper exists for **both** successful and failed diagnostic capture.

The original typed application error is never replaced by instrumentation
failure.

Required access:

```rust
error()
capture()
output()        -> Result<&SnafuBridgeOutput, &SnafuBridgeError>
report()        -> Result<&DiagnosticReport, &SnafuBridgeError>
graph()         -> Result<&DiagnosticRelationshipGraph, &SnafuBridgeError>
capture_error() -> Option<&SnafuBridgeError>
into_error()
into_parts()    -> (E, Result<SnafuBridgeOutput, SnafuBridgeError>)
```

Successful capture remains the normal path and exposes the complete report and
graph.

If metadata construction, strict root mapping, source traversal, or bridge
assembly fails, the wrapper retains:

```text
the original concrete E
the structured SnafuBridgeError
```

This means `result.diagprint(...)?` remains ergonomic and never loses the
application error merely because diagnostic instrumentation failed.

`Display` continues to delegate to the application error rather than replacing
its message with instrumentation details.

`Debug` may expose capture success/failure state but must avoid dumping
privacy-sensitive diagnostic contents.

Recommended ergonomics:

```text
AsRef<E>
Deref<Target = E> if it remains unsurprising
Display delegates to E
Debug exposes both typed error and diagnostic summary without dumping secrets
```

`std::error::Error` behavior must preserve a useful source chain.

If `E: ErrorCompat`, `CapturedSnafuError<E>` should preserve/delegate compatible
backtrace access where the trait permits.

The wrapper exists so a caller can keep matching/downcasting the SNAFU error
while also carrying diagprint's report/graph when capture succeeds.

Capture failure is intentionally **data on the wrapper**, not a second error
type that would force callers to choose between their domain error and
diagprint instrumentation state.

## Strict extension-trait contract

The primary extension-trait path is **lifecycle-ready**.

That means it requires stable root identity.

Two ways to satisfy that:

```text
1. E implements SnafuDiagnostic
2. caller supplies an explicit mapper/profile that produces stable root metadata
```

If the extension path cannot establish root identity, the
`CapturedSnafuError<E>` retains the original `E` and stores a structured
`MissingRootIdentity` capture error rather than silently falling back to
message-based identity.

The lower-level `SnafuBridge` may still offer best-effort conversion for
exploration/debugging, but the ergonomic capture traits stay strong.

## Custom mapper/profile path

Downstream users cannot always implement `SnafuDiagnostic` directly for a type
they do not own.

E2 therefore keeps an explicit mapper/profile route.

Proposed:

```rust
pub struct SnafuErrorView<'a> {
    error: &'a (dyn std::error::Error + 'static),
    depth: usize,
    is_root: bool,
}

pub trait SnafuErrorMapper {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError>;
}
```

Semantics:

```text
Some(metadata)
    explicitly mapped node

None
    adapter fallback / unmapped node
```

A mapper can downcast foreign or application-specific errors and map fields.

This route supports customization without requiring a diagprint-snafu proc
macro.

The library should provide a closure-friendly mapper adapter if practical.

## Result extension trait

Primary trait:

```rust
pub trait DiagprintResultExt<T, E>: Sized {
    // ...
}
```

Required method families:

### Capture an already-constructed custom SNAFU error

Conceptual:

```rust
fn diagprint(
    self,
    reporter: &Reporter,
) -> Result<T, CapturedSnafuError<E>>
where
    E: SnafuDiagnostic;
```

Custom configuration:

```rust
fn diagprint_with<M>(
    self,
    reporter: &Reporter,
    mapper: &M,
) -> Result<T, CapturedSnafuError<E>>
where
    E: Error + ErrorCompat + 'static,
    M: SnafuErrorMapper + ?Sized;
```

### Apply SNAFU context and capture

Mirror SNAFU's ResultExt concepts:

```rust
fn diagprint_context<C, E2>(
    self,
    reporter: &Reporter,
    context: C,
) -> Result<T, CapturedSnafuError<E2>>
where
    C: snafu::IntoError<E2, Source = E>,
    E2: SnafuDiagnostic;
```

Lazy:

```rust
fn diagprint_with_context<F, C, E2>(
    self,
    reporter: &Reporter,
    context: F,
) -> Result<T, CapturedSnafuError<E2>>
where
    F: FnOnce(&mut E) -> C,
    C: snafu::IntoError<E2, Source = E>,
    E2: SnafuDiagnostic;
```

This must preserve SNAFU 0.9's lazy closure access to the underlying error.

Exact parameter order may be refined for ergonomics.

## Option extension trait

Primary trait:

```rust
pub trait DiagprintOptionExt<T>: Sized {
    // ...
}
```

Option capture is context-driven because `None` has no error value.

Required methods mirror SNAFU OptionExt:

```rust
fn diagprint_context<C, E>(
    self,
    reporter: &Reporter,
    context: C,
) -> Result<T, CapturedSnafuError<E>>
where
    C: snafu::IntoError<E, Source = snafu::NoneError>,
    E: SnafuDiagnostic;
```

Lazy:

```rust
fn diagprint_with_context<F, C, E>(
    self,
    reporter: &Reporter,
    context: F,
) -> Result<T, CapturedSnafuError<E>>
where
    F: FnOnce() -> C,
    C: snafu::IntoError<E, Source = snafu::NoneError>,
    E: SnafuDiagnostic;
```

A mapper/profile variant should exist for custom target errors that cannot
implement `SnafuDiagnostic`.

## Futures feature and async traits

Async extension traits are behind:

```text
feature = "futures"
```

No Tokio dependency.

Primary traits:

```rust
pub trait DiagprintTryFutureExt: futures::TryFuture + Sized {
    // ...
}

pub trait DiagprintTryStreamExt: futures::TryStream + Sized {
    // ...
}
```

They should be re-exportable from a small `diagprint_snafu::prelude` without
method-name collision with SNAFU because E2 methods use `diagprint*` names.

## Future extension trait

Required method families:

```text
diagprint
diagprint_with
diagprint_context
diagprint_with_context
```

Semantics:

```text
Ok value:
  passes through unchanged

Err:
  optional SNAFU context construction
  typed/custom metadata conversion
  CapturedSnafuError<E or E2>
```

The returned future must be lazy.

Do not eagerly poll or allocate/box merely to implement the trait.

Prefer concrete combinator/wrapper types or zero-cost `impl Future` surfaces
compatible with Rust 1.85.

### Async location doctrine

When `diagprint_context` constructs SNAFU context inside a future combinator,
implicit SNAFU `Location` follows SNAFU's own async semantics and may identify
poll-time/combinator location.

E2 must document this.

`diagprint_with_context` must allow applications to provide explicit SNAFU
location data through their context selector when exact construction location
matters.

Diagprint identity must not depend on poll location.

## Stream extension trait

Required method families:

```text
diagprint
diagprint_with
diagprint_context
diagprint_with_context
```

For each stream item:

```text
Ok(item)
  passes through

Err(error)
  is context-enriched if requested
  is independently converted
  becomes CapturedSnafuError<...>
```

Streams may yield multiple errors depending on the source stream semantics.

E2 must not accumulate all stream errors into one hidden global report.

Each error item owns its own `SnafuBridgeOutput`.

This preserves streaming behavior and avoids unbounded diagnostic accumulation.

No mandatory boxing.

## Strong Whatever path

`Whatever` remains important but uses the same extension architecture.

Add:

```rust
pub struct WhateverDiagnosticContext {
    identity: SnafuIdentity,
    code: Option<SnafuCode>,
    message: String,
    // optional presentation metadata
}
```

Conceptual construction:

```rust
WhateverDiagnosticContext::new(
    SnafuIdentity::new("config.read")?,
    "reading configuration",
)
```

Result/Option/Future/Stream extension traits should support strong Whatever
context methods where practical:

```text
diagprint_whatever_context
diagprint_with_whatever_context
```

These methods combine:

```text
SNAFU FromString / Whatever construction
+ required explicit identity
+ diagprint capture
```

The methods must not derive identity from the Whatever message.

`WhateverLocal` receives equivalent semantics.

If exact generic support for user-defined `#[snafu(whatever)]` error types
would make the API ambiguous, E2 may first guarantee the premade
`Whatever`/`WhateverLocal` path and require custom whatever-enabled error types
to use `SnafuDiagnostic` + native SNAFU construction.

## Source-chain topology

Use stable `Error::source`.

For:

```text
leaf -> context -> outer
```

graph edges:

```text
source
  -- ContributesTo / SourceChain -->
wrapper
```

Required:

```text
kind:     DiagnosticRelationshipKind::ContributesTo
evidence: DiagnosticRelationshipEvidence::SourceChain
producer: "snafu"
```

This is structural evidence, not proven root cause.

## Source-node customization

Root custom metadata comes from `SnafuDiagnostic` or explicit mapper/profile.

Source nodes may be:

```text
another SNAFU custom error
std error
foreign error
Whatever / WhateverLocal
```

A supplied mapper can stabilize known source nodes via downcast.

Unmapped source nodes remain representable as presentation-only diagnostics.

Do not fabricate stable source identity from their message, depth or address.

## Backtrace policy

Proposed:

```rust
pub enum SnafuBacktracePolicy {
    Omit,
    DisplayText,
}
```

Default:

```text
Omit
```

`ErrorCompat` is not a general dyn-error reflection mechanism.

Generic backtrace extraction is guaranteed for the concrete root error where
the static type implements `ErrorCompat`.

A custom mapper may intentionally handle a known concrete source type.

Do not use unstable provider APIs to discover backtraces dynamically.

Backtrace never participates in canonical identity.

## Text/privacy policy

Proposed:

```rust
pub enum SnafuTextPolicy {
    Display,
    RedactUnmapped,
}
```

Default:

```text
Display
```

`RedactUnmapped` affects fallback presentation only.

It must not change identity or source topology.

Custom `SnafuDiagnosticMetadata` is application-authored and therefore remains
under application control.

## Error model

Proposed adapter error variants include:

```rust
Bridge(BridgeError)
InvalidIdentity { ... }
InvalidCode { ... }
MissingRootIdentity
SourceCycle { depth: usize }
SourceDepthExceeded { limit: usize }
```

Exact shape may be refined.

Fail closed on malformed topology or a strong extension capture lacking stable
root identity.

## Statistics

`SnafuBridgeOutput` reuses `BridgeBuildStats`.

Potential adapter counters:

```text
mapped_nodes
unmapped_nodes
source_nodes
source_relationships
whatever_nodes
whatever_local_nodes
redacted_nodes
backtraces_included
```

Only retain deterministic, useful counters.

## Prelude

Provide an opt-in prelude:

```rust
use diagprint_snafu::prelude::*;
```

Expected exports:

```text
DiagprintResultExt as _
DiagprintOptionExt as _

with feature futures:
  DiagprintTryFutureExt as _
  DiagprintTryStreamExt as _
```

Do not re-export all of SNAFU's prelude.

The crate should compose with:

```rust
use snafu::prelude::*;
use diagprint_snafu::prelude::*;
```

without ambiguous method names.

## Non-goals

E2 will not:

- replace SNAFU;
- add SNAFU to diagprint core;
- add Tokio;
- build an executor;
- parse `snafu::Report` Display/Debug;
- inspect private SNAFU fields;
- depend on unstable provider APIs;
- auto-reflect arbitrary enum fields;
- generate a new derive macro in E2;
- derive identity from messages, Debug, backtraces, depth or addresses;
- accept arbitrary runtime strings as strong codes;
- claim source ancestry proves root cause;
- force backtrace capture;
- hide unbounded stream diagnostic accumulation;
- require boxing for every future/stream adapter;
- modify protected diagprint schemas;
- modify v0.8.0 artifacts/tag;
- implement E3/E4.

## Compatibility

Protected:

```text
diagprint.canonical/v1
diagprint.history.run/v2
diagprint.history.head/v1
diagprint.relationship.graph/v1
diagprint.relationship.snapshot/v1
diagprint.forensics.git-provenance/v1
diagprint.remediation.evidence/v1
diagprint.forensics.remediation-replay/v1
diagprint.capsule/v1
diagprint-bridge public v0.8 architecture
```

Rust 1.85 remains the workspace MSRV.

## Expected file boundary

New:

```text
crates/diagprint-snafu/Cargo.toml
crates/diagprint-snafu/README.md

crates/diagprint-snafu/src/lib.rs
crates/diagprint-snafu/src/metadata.rs
crates/diagprint-snafu/src/bridge.rs
crates/diagprint-snafu/src/captured.rs
crates/diagprint-snafu/src/ext/result.rs
crates/diagprint-snafu/src/ext/option.rs

with feature futures:
crates/diagprint-snafu/src/ext/future.rs
crates/diagprint-snafu/src/ext/stream.rs

tests:
crates/diagprint-snafu/tests/custom_errors.rs
crates/diagprint-snafu/tests/result_ext.rs
crates/diagprint-snafu/tests/option_ext.rs
crates/diagprint-snafu/tests/whatever.rs
crates/diagprint-snafu/tests/privacy.rs
crates/diagprint-snafu/tests/future_ext.rs
crates/diagprint-snafu/tests/stream_ext.rs
```

Modified:

```text
Cargo.toml
Cargo.lock
README.md
CHANGELOG.md
docs/ecosystem-bridges-roadmap.md
.plans/E2-snafu.plan.md
```

Possible only if required:

```text
scripts/gate.sh
.github/workflows/ci.yml
```

Protected unless plan revision:

```text
src/
tests/
crates/diagprint-bridge/src/
crates/diagprint-bridge/tests/
crates/diagprint-error-stack/src/
crates/diagprint-error-stack/tests/
```

If `diagprint-bridge` proves insufficient, stop and revise the plan.

## Test-first matrix

### Custom enum variant mapping

Use a multi-variant `#[derive(Snafu)]` enum.

Prove each variant can select independently:

```text
SnafuIdentity
SnafuCode
Severity
message normalization
notes/help/labels
```

Dynamic variant fields remain accessible to the implementation.

### Custom struct error

Use a derived SNAFU struct and prove the same metadata contract works without an
enum.

### Variant identity stability

Same variant + same identity/code + different runtime fields:

```text
same logical fingerprint
different presentation/content digest as appropriate
```

### Variant distinction

Two variants with different identities produce different fingerprints even if
their Display text is identical.

### Code stability

Same identity + same code + different presentation => same fingerprint.

Same identity + different code => different fingerprint.

Invalid strong codes rejected.

### CapturedSnafuError preservation

Prove successful capture:

```text
original concrete E is retained
error() returns typed E
into_error() recovers E
output/report/graph are available
Display remains useful
source chain remains useful
```

Also prove failed capture:

```text
original concrete E is still retained
capture_error() exposes structured SnafuBridgeError
output/report/graph return that capture error
into_error() still recovers E
Display still represents E rather than instrumentation failure
```

### Result .diagprint

```text
Ok passes through without conversion
Err custom error is captured
stable root identity required
typed error preserved
```

### Result context

Mirror SNAFU:

```text
diagprint_context
diagprint_with_context
```

Prove lazy context closure is invoked only for Err and receives mutable access
to the original error under SNAFU 0.9 semantics.

### Option context

Prove `Some` does not build context.

Prove `None` builds custom SNAFU error through `IntoError<Source=NoneError>`,
then captures it.

### Foreign/non-local custom mapping

Use a mapper/profile against an error type that cannot use the direct
`SnafuDiagnostic` convenience path.

Prove stable root mapping and typed downcast customization.

### Missing root identity

Strong extension capture with no stable root identity records
`MissingRootIdentity` inside `CapturedSnafuError<E>`.

The original typed error remains recoverable.

No message-derived fallback.

### Whatever dynamic-message stability

Different Whatever messages + same strong identity/code => same fingerprint.

### Whatever source chain

Strong Whatever wrapping a standard error preserves SourceChain graph evidence.

### WhateverLocal

Equivalent identity/capture semantics without imposing Send + Sync.

### Result strong Whatever context

Prove identity and message are supplied separately and `FromString` source
construction preserves the underlying error.

### Future pass-through

With `futures` feature:

```text
Ok future output unchanged
Err captured lazily
future is not polled by construction
```

### Future context

`diagprint_context` and `diagprint_with_context` preserve SNAFU target error
construction and typed metadata.

Document/test poll-time implicit-location behavior without making unstable exact
line assertions across combinators.

### Stream pass-through

```text
Ok items unchanged
each Err independently captured
stream remains lazy
```

### Stream multiple errors

A stream yielding more than one error creates independent
`CapturedSnafuError` values.

No hidden global accumulation.

### Stream context

Context selector applies independently to each Err item.

Lazy closure receives each source error according to SNAFU/futures semantics.

### Async no-boxing contract

Representative future/stream extension methods compile without requiring user
boxing.

### Prelude composition

```rust
use snafu::prelude::*;
use diagprint_snafu::prelude::*;
```

compiles without ambiguous extension method names.

### Source topology

Source -> wrapper edges use:

```text
ContributesTo
SourceChain
producer snafu
```

### Cycle/depth defense

Malformed source cycles terminate deterministically.

### Privacy

Sentinel secrets in unmapped error text/backtrace are absent under:

```text
RedactUnmapped
Omit backtrace
```

Stable identity/topology unchanged.

### Backtrace

Root backtrace omitted by default.

Opt-in works without changing fingerprint.

### Regression

All `diagprint-bridge` and `diagprint-error-stack` tests stay green unchanged.

Protected schemas unchanged.

### MSRV

Rust 1.85 passes both:

```text
default feature set
features = futures
```

## Implementation sequence

### E2A — Custom error model + Result/Option extensions

Deliver:

```text
diagprint-snafu crate skeleton
SnafuIdentity
SnafuCode
SnafuDiagnosticMetadata
SnafuDiagnostic
SnafuErrorMapper
SnafuBridge
CapturedSnafuError<E>
DiagprintResultExt
DiagprintOptionExt
custom enum/struct tests
Result/Option context tests
source graph foundation
strict root identity
```

Also establish the shared strong Whatever metadata/context types needed by later
extension families.

Gate:

```bash
cargo test -p diagprint-snafu --test custom_errors
cargo test -p diagprint-snafu --test result_ext
cargo test -p diagprint-snafu --test option_ext
cargo test -p diagprint-snafu --test whatever
cargo check -p diagprint-snafu --all-targets
cargo clippy -p diagprint-snafu --all-targets -- -D warnings
./scripts/gate.sh fast
```

Commit E2A and require exact CI success before E2B.

### E2B — Future/Stream extension traits

Enable opt-in `futures`.

Deliver:

```text
DiagprintTryFutureExt
DiagprintTryStreamExt
lazy context mapping
CapturedSnafuError propagation through async flows
multiple stream error handling
async location doctrine docs/tests
prelude futures exports
no mandatory boxing
```

Gate:

```bash
cargo test -p diagprint-snafu --features futures --test future_ext
cargo test -p diagprint-snafu --features futures --test stream_ext
cargo test -p diagprint-snafu --features futures
cargo clippy -p diagprint-snafu --all-targets --features futures -- -D warnings
./scripts/gate.sh fast
```

Commit E2B and require exact CI success before E2C.

### E2C — Advanced customization + privacy/backtrace + docs

Deliver:

```text
foreign/non-local mapper/profile coverage
text policy
backtrace policy
strong Whatever extension methods across supported flow types
adapter statistics
privacy tests
README
CHANGELOG
ecosystem roadmap update
full extension-trait examples
```

Gate:

```bash
cargo test -p diagprint-snafu
cargo test -p diagprint-snafu --features futures
cargo test -p diagprint-bridge
cargo test -p diagprint-error-stack
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
./scripts/gate.sh full
```

Commit E2C and require exact CI success.

### E2 closure

After exact E2C CI:

```text
record E2A/E2B/E2C commits and runs
verify protected schemas unchanged
verify SNAFU dependency isolation
verify default + futures feature matrices
verify Rust 1.85
mark plan Complete
remove .plans/ACTIVE
separate docs/planning closure commit
exact closure CI
```

## CI discipline

```text
Draft
  -> explicit approval
Approved
  -> commit Approved plan
  -> exact plan CI success
E2A
  -> exact CI success
E2B
  -> exact CI success
E2C
  -> exact CI success
closure
  -> exact CI success
```

No implementation before Approved plan commit + CI.

## Acceptance criteria

E2 is complete only when:

- `diagprint-snafu` is isolated from core;
- custom `#[derive(Snafu)]` enum/struct errors have a rich typed customization
  contract;
- custom variants can define stable identity/code/severity/presentation and
  structured metadata independently;
- `CapturedSnafuError<E>` always preserves the concrete SNAFU error and
  carries either successful diagprint output or a structured capture error;
- Result extension traits support capture and SNAFU context construction;
- Option extension traits support SNAFU None-context construction;
- Future and Stream extension traits exist behind opt-in `futures`;
- async adapters remain lazy and do not require universal boxing;
- stream errors are captured independently without hidden accumulation;
- extension methods compose with SNAFU's own prelude without collisions;
- a mapper/profile route supports further customization and foreign error types;
- strong extension capture requires stable root identity;
- Whatever/WhateverLocal keep explicit identity separate from message text;
- canonical code interaction is honored through `SnafuCode`;
- source relationships remain deterministic M4 SourceChain evidence;
- backtrace is omitted by default and never identity;
- privacy/redaction does not change identity/topology;
- unstable SNAFU provider APIs and Report parsing are not used;
- E1/bridge regressions remain green;
- protected schemas remain unchanged;
- Rust 1.85 passes default and futures feature matrices;
- exact E2A/E2B/E2C/closure CI all succeed.

## Completion record

```text
Draft baseline:
634dee04dc74ab31e359a232dc6cf00d4b65cc52

Approved E2 plan commit:
e630a596c5221418598d8dc7e2c747333129e32f
Approved E2 plan CI:
35282386610
Approved E2 plan result:
success

E2A custom errors + sync extensions commit:
E2A CI:
E2A result:

E2B future/stream extensions commit:
E2B CI:
E2B result:

E2C customization/privacy/docs commit:
E2C CI:
E2C result:

E2 closure commit:
E2 closure CI:
E2 closure result:

Notes:
- Approved-plan CI 35282386610 succeeded on exact commit
  e630a596c5221418598d8dc7e2c747333129e32f.
- A pre-implementation contract review found that SnafuDiagnostic metadata,
  strict root mapping, source traversal, and bridge assembly are fallible while
  the ergonomic extension surface returns CapturedSnafuError<E>.
- The Approved plan therefore defines CapturedSnafuError<E> as retaining the
  original E plus Result<SnafuBridgeOutput, SnafuBridgeError>. Instrumentation
  failure never replaces the application error.
- v0.8.0 fully closed before E2.
- custom SNAFU errors are the primary E2 design target.
- Result/Option/Future/Stream extension traits are primary product surfaces.
- Whatever remains strong but no longer dominates the adapter design.
- futures support is opt-in and adds no Tokio dependency.
- E2 reuses diagprint-bridge rather than modifying core.
```
