# diagprint-otel

Privacy-aware OpenTelemetry integration for the `diagprint` diagnostics
lifecycle framework.

`diagprint-otel` maps structured diagnostics to OpenTelemetry events without
owning exporter, collector, SDK, batching, sampling, or runtime configuration.

## Highlights

- privacy-aware diagnostic events
- explicit text export policy
- explicit source-location export policy
- explicit structured-attribute export policy
- OpenTelemetry span integration
- tracing-opentelemetry integration
- no collector or exporter lock-in

Free-form diagnostic text is not exported in plaintext by default.

Arbitrary structured diagnostic attributes are omitted unless explicitly
enabled.

## Installation

    [dependencies]
    diagprint = "0.7"
    diagprint-otel = "0.7"

## Main API

The primary types are:

- `TelemetryAdapter`
- `TelemetryEvent`
- `TelemetryPolicy`
- `TextExport`
- `LocationExport`
- `AttributeExport`

The crate also re-exports `opentelemetry` and `tracing-opentelemetry`.

## Privacy model

Applications explicitly choose what diagnostic content may cross the telemetry
boundary. Sensitive free-form text, locations, and arbitrary attributes are not
implicitly exposed.

## MSRV

Rust 1.85 or newer.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
