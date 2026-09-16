# diagprint-axum

Privacy-safe Axum integration for the `diagprint` diagnostics lifecycle
framework.

`diagprint-axum` adapts structured diagnostics and Axum request failures into
HTTP responses without introducing Axum or an async runtime dependency into
the core `diagprint` crate.

## Architecture

The dependency direction is intentional:

    Axum application
          |
          v
    diagprint-axum
          |
          v
      diagprint

Core diagprint does not depend on Axum.

`diagprint-axum` begins the v0.8 ecosystem layer while core remains on the
stable v0.7.x line except for fixes.

## Installation

    [dependencies]
    diagprint = "0.7"
    diagprint-axum = "0.8"

## Privacy model

HTTP responses are externalization boundaries.

A diagnostic can contain internal messages, paths, causes, attributes,
remediation data, process metadata, and other information that should not
automatically reach a client.

The default response exposes only:

- HTTP status;
- a generic client-safe message;
- the diagnostic report ID for correlation.

The default response does not expose:

- internal diagnostic messages;
- diagnostic codes;
- source locations;
- notes;
- help;
- causes;
- structured attributes;
- suggestions;
- remediation information;
- hostname;
- process ID;
- session ID.

## Basic diagnostic response

    use axum::http::StatusCode;
    use diagprint::Reporter;
    use diagprint_axum::DiagnosticResponse;

    let reporter = Reporter::builder()
        .application("api")
        .build()?;

    let diagnostic = reporter
        .error("database connection failed")
        .code("database.unavailable");

    let response = DiagnosticResponse::new(
        StatusCode::SERVICE_UNAVAILABLE,
        diagnostic,
    );

The client receives a generic server-error message rather than the internal
database diagnostic.

## Explicit exposure

Selected information can be deliberately made public:

    use diagprint_axum::ResponsePolicy;

    let policy = ResponsePolicy::default()
        .expose_diagnostic_message(true)
        .expose_diagnostic_code(true);

    let response = DiagnosticResponse::new(status, diagnostic)
        .with_policy(policy);

This should only be used when those messages and codes are intentionally part
of the application's public HTTP contract.

## Axum rejection adapters

`diagprint-axum` can convert common Axum extractor failures into structured
diagprint diagnostics while preserving Axum's HTTP status.

Supported rejection families currently include:

- JSON;
- path parameters;
- query strings;
- forms;
- required extensions.

Use `Result<Extractor, Rejection>` in the handler and convert failures through
`AxumRejectionExt`.

For example:

    use axum::{
        Json,
        extract::rejection::JsonRejection,
        http::StatusCode,
    };

    use diagprint::Reporter;
    use diagprint_axum::{
        AxumRejectionExt,
        DiagnosticResult,
    };

    async fn handler(
        payload: Result<Json<serde_json::Value>, JsonRejection>,
    ) -> DiagnosticResult<StatusCode> {
        let reporter = Reporter::builder()
            .application("api")
            .build()
            .expect("reporter should build");

        match payload {
            Ok(_) => Ok(StatusCode::NO_CONTENT),
            Err(rejection) => {
                Err(rejection.to_diagnostic_response(&reporter))
            }
        }
    }

The internal diagnostic retains rejection detail and structured metadata such
as:

- diagnostic code;
- HTTP status;
- HTTP status class;
- rejection category.

The client response remains redacted by default.

## Rejection severity

Client-side HTTP rejection statuses are represented as warning diagnostics.

Server-side HTTP rejection statuses are represented as error diagnostics.

For example, a missing required Axum `Extension` is a server configuration
failure and therefore becomes an error diagnostic while the client still sees
only the generic server-error response.

## Status preservation

diagprint-axum uses Axum's public rejection status instead of recreating
Axum's status mapping.

This avoids coupling the adapter to every current rejection enum variant and
allows Axum to add future variants to its non-exhaustive rejection enums
without forcing diagprint-axum to duplicate their internal matching logic.

## Emission and persistence

Creating a `DiagnosticResponse` does not automatically:

- print the diagnostic;
- write it to disk;
- send telemetry;
- persist an artifact;
- create history;
- emit tracing events.

The application retains control over the internal lifecycle.

This separation prevents an HTTP adapter from silently introducing
observability side effects.

## Main API

The primary response types are:

- `DiagnosticResponse`
- `ResponsePolicy`
- `ClientErrorEnvelope`
- `ClientErrorBody`
- `DiagnosticResponseExt`
- `DiagnosticResult`

The rejection API consists of:

- `AxumRejectionExt`
- `RejectionKind`

## Development gates

Contributors can validate this crate independently:

    ./scripts/diagprint-axum-gates quick

For the full development contract:

    ./scripts/diagprint-axum-gates full

Immediately before a release:

    ./scripts/diagprint-axum-gates release

The release mode performs packaging and a crates.io publish dry-run.

It never performs the real publication.

The workspace-wide standard is documented in:

    docs/COMPANION_CRATE_STANDARD.md

## Current scope

The v0.8 adapter currently provides:

- privacy-safe diagnostic HTTP responses;
- correlation through report IDs;
- explicit public-message/code policy;
- JSON rejection diagnostics;
- path rejection diagnostics;
- query rejection diagnostics;
- form rejection diagnostics;
- required-extension rejection diagnostics.

Likely future work includes:

- DiagnosticReport responses;
- request correlation middleware;
- tracing/request context;
- application-error conversion;
- RFC Problem Details;
- middleware-based internal diagnostic emission;
- additional extractor adapters.

Those additions must preserve the same privacy and dependency boundaries.

## MSRV

Rust 1.85 or newer.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
