# diagprint-snafu

Structured SNAFU interoperability for diagprint.

`diagprint-snafu` preserves application-owned SNAFU error types while attaching
diagprint lifecycle output:

```text
SNAFU typed error
      |
      +--> stable application identity/code
      +--> diagprint DiagnosticReport
      +--> M4 source-chain graph
      +--> optional privacy/backtrace policy
      +--> CapturedSnafuError<E>
```

The original concrete error is retained even when diagnostic instrumentation
fails.

## Custom typed SNAFU errors

Implement `SnafuDiagnostic` on application-owned SNAFU errors and choose stable
identity variant-by-variant:

```rust
use diagprint_snafu::{
    SnafuBridgeError, SnafuCode, SnafuDiagnostic, SnafuDiagnosticMetadata,
    SnafuIdentity,
};
use snafu::Snafu;

#[derive(Debug, Snafu)]
enum AppError {
    #[snafu(display("Invalid port {value}"))]
    InvalidPort { value: u16 },
}

impl SnafuDiagnostic for AppError {
    fn diagprint_metadata(
        &self,
    ) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        match self {
            Self::InvalidPort { value } => Ok(
                SnafuDiagnosticMetadata::new(
                    SnafuIdentity::new("config.port.invalid")?,
                    format!("Invalid port {value}"),
                )
                .code(SnafuCode::new("CONFIG_PORT")?),
            ),
        }
    }
}
```

Dynamic message text is not used as stable identity.

## Result and Option

```rust
use diagprint_snafu::prelude::*;

let value = fallible_operation().diagprint(&reporter)?;
let value = maybe_value.diagprint_context(&reporter, MissingValueSnafu)?;
```

Mapper variants support errors that cannot implement `SnafuDiagnostic`.

## Foreign errors and capture profiles

`SnafuCaptureProfile` combines a source mapper with presentation policy:

```rust
use diagprint_snafu::{
    SnafuBacktracePolicy, SnafuCaptureProfile, SnafuTextPolicy, snafu_mapper,
};

let mapper = snafu_mapper(|view| {
    // Downcast known foreign source types and return stable metadata.
    Ok(None)
});

let profile = SnafuCaptureProfile::new(mapper)
    .text_policy(SnafuTextPolicy::RedactUnmapped)
    .backtrace_policy(SnafuBacktracePolicy::Omit);
```

Policies do not claim stronger evidence semantics.

`RedactUnmapped` replaces fallback source `Display` text with a fixed marker
while preserving source-chain topology.

Backtrace policy defaults to `Omit`. `DisplayText` copies the concrete root
SNAFU backtrace into presentation-only note text. Backtrace text never
participates in stable identity.

## Strong Whatever / WhateverLocal

The strong Whatever helpers require explicit `WhateverDiagnosticContext`:

```rust
use diagprint_snafu::{
    SnafuIdentity, WhateverDiagnosticContext,
    prelude::*,
};

let result = std::fs::read_to_string("config.toml")
    .diagprint_whatever_context(
        &reporter,
        WhateverDiagnosticContext::new(
            SnafuIdentity::new("config.read")?,
            "reading configuration",
        ),
    );
```

The message is used to build SNAFU's `Whatever`; diagprint identity comes only
from the explicit `SnafuIdentity`/`SnafuCode`.

`WhateverLocal` equivalents support non-`Send` / non-`Sync` source errors.

Result, Option, Future, and Stream surfaces provide eager and lazy strong
Whatever context methods.

## Futures feature

Enable:

```toml
diagprint-snafu = { version = "0.8", features = ["futures"] }
```

The feature adds:

- `DiagprintTryFutureExt`;
- `DiagprintTryStreamExt`;
- lazy direct capture;
- lazy SNAFU typed context construction;
- strong Whatever/WhateverLocal async context;
- independent capture for each stream error;
- no Tokio dependency;
- no mandatory Future/Stream boxing.

SNAFU's implicit async location follows poll/combinator execution rather than
necessarily identifying combinator construction. diagprint identity is
independent of that location.

## Evidence doctrine

Source relationships are emitted as:

```text
kind:     ContributesTo
evidence: SourceChain
producer: snafu
```

This records structural source-chain evidence. It does not claim root cause or
remediation causation.

## Defaults

```text
unmapped source text: Display
root backtrace:       Omit
```

Privacy-sensitive applications can opt into `RedactUnmapped`.
