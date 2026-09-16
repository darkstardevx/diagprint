# diagprint-axum

Privacy-safe Axum integration for the `diagprint` diagnostics lifecycle
framework.

`diagprint-axum` adapts structured diagprint diagnostics to HTTP responses
without introducing Axum or an async runtime dependency into the core
`diagprint` crate.

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

`diagprint-axum` begins the v0.8 ecosystem layer while the core crate remains
on the stable v0.7.x line except for fixes.

## First principles

HTTP responses are an externalization boundary.

Diagnostics may contain internal messages, paths, causes, attributes,
remediation data, process metadata, or other information that should not
automatically be sent to a client.

For that reason, client responses are redacted by default.

The default response exposes:

- HTTP status
- a generic client-safe message
- the diagnostic report ID for correlation

The default response does not expose:

- the diagnostic message
- the diagnostic code
- source locations
- notes
- help
- causes
- attributes
- suggestions
- remediation data
- hostname
- process ID
- session ID

Applications may explicitly opt into exposing diagnostic messages or codes.

## Installation

    [dependencies]
    diagprint = "0.7"
    diagprint-axum = "0.8"

## Basic usage

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

By default the client receives a generic message rather than
`"database connection failed"`.

## Explicit exposure

An application may deliberately expose selected diagnostic information:

    use diagprint_axum::ResponsePolicy;

    let policy = ResponsePolicy::default()
        .expose_diagnostic_message(true)
        .expose_diagnostic_code(true);

    let response = DiagnosticResponse::new(status, diagnostic)
        .with_policy(policy);

This should normally be reserved for diagnostics whose messages and codes are
part of the application's public API contract.

## Emission and persistence

`diagprint-axum` does not automatically print, persist, transmit, or log the
underlying diagnostic.

The application remains responsible for sending the internal diagnostic to its
chosen diagprint sink, reporter, telemetry pipeline, artifact store, or other
lifecycle destination.

This separation prevents the HTTP adapter from silently creating side effects.

## Current scope

The initial v0.8 surface provides:

- `DiagnosticResponse`
- `ResponsePolicy`
- `ClientErrorEnvelope`
- `ClientErrorBody`
- `DiagnosticResponseExt`
- `DiagnosticResult`

Future work may add:

- `DiagnosticReport` responses
- Axum rejection adapters
- request correlation middleware
- tracing/request-context capture
- configurable RFC Problem Details responses
- structured application-error conversion
- middleware-based internal diagnostic emission

Those additions must preserve the same privacy and dependency boundaries.

## MSRV

Rust 1.85 or newer.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
