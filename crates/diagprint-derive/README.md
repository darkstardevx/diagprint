# diagprint-derive

Procedural derive support for [`diagprint`](https://crates.io/crates/diagprint).

This crate contains only compile-time macro expansion. Runtime diagnostic
behavior remains in the main `diagprint` crate.

Most users should enable the `derive` feature on `diagprint` rather than depend
on `diagprint-derive` directly:

~~~toml
[dependencies]
diagprint = { version = "0.7", features = ["derive"] }
~~~

Then derive `DiagnosticMetadata` through the re-exported `Diagnostic` macro:

~~~rust
use diagprint::Diagnostic;

#[derive(Debug, Diagnostic)]
#[diag(
    code = "APP-001",
    severity = error,
    help = "check the application configuration"
)]
struct ConfigurationError;
~~~

## Supported metadata

The derive macro supports type- and variant-level diagnostic metadata including:

- diagnostic codes;
- severity;
- help text;
- repeated notes;
- structured suggestions;
- primary and secondary source labels;
- label messages;
- label lengths referencing named fields;
- enum-level defaults with variant overrides.

## Safety behavior

The macro intentionally fails closed when metadata would imply unsafe automatic
remediation.

In particular, derive-generated `machine_applicable` suggestions are rejected
because guarded edit generation is not currently supported by the derive macro.
Automatic remediation must carry explicit guarded edits through diagprint's
runtime remediation APIs.

## Compile-time validation

The crate validates malformed diagnostic metadata at compile time, including:

- unsupported struct and enum forms;
- duplicate scalar options;
- invalid severity values;
- unsupported diagnostic options;
- malformed suggestions;
- invalid applicability values;
- conflicting label kinds;
- duplicate label metadata;
- unsupported field metadata.

These compiler diagnostics are covered by UI/compile-fail regression tests.

## MSRV

`diagprint-derive` follows diagprint's minimum supported Rust version:

**Rust 1.85**

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
