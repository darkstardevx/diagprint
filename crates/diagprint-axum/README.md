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

## RFC 9457 Problem Details

`diagprint-axum` can render a `DiagnosticResponse` as an
[RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457.html)
document.

Problem Details changes the HTTP representation, not the privacy boundary.
The existing `ResponsePolicy` remains the authority that decides whether an
internal diagnostic message, diagnostic code, or report ID may be exposed.

The response media type is:

    application/problem+json

A default server-error response looks like:

    {
      "type": "about:blank",
      "title": "Internal Server Error",
      "status": 500,
      "detail": "Internal server error.",
      "report_id": "550e8400-e29b-41d4-a716-446655440000",
      "request_id": "req-01JEXAMPLE"
    }

The internal diagnostic message and diagnostic code remain absent by default.
For example, an internal diagnostic such as:

    database password secret should never leak

is not copied into `detail`. The generic client-safe fallback selected by
`ResponsePolicy` is used instead.

Create a Problem Details response from an existing diagnostic response with:

    use axum::http::StatusCode;
    use diagprint::Reporter;
    use diagprint_axum::{
        DiagnosticResponse,
        ProblemDetailsResponseExt,
    };

    let reporter = Reporter::builder()
        .application("api")
        .build()?;

    let diagnostic = reporter
        .error("database connection failed")
        .code("database.unavailable");

    let response = DiagnosticResponse::new(
        StatusCode::SERVICE_UNAVAILABLE,
        diagnostic,
    )
    .into_problem_details();

The default problem type is `about:blank`. Under RFC 9457, `about:blank`
indicates that the problem has no application-specific semantics beyond the
HTTP status code.

### Custom problem types

Applications can define stable public problem types with
`ProblemDetailsPolicy`:

    use diagprint_axum::{
        ProblemDetailsPolicy,
        ProblemDetailsResponseExt,
    };

    let problem_policy = ProblemDetailsPolicy::default()
        .with_type_uri("https://api.example.com/problems/invalid-order")
        .with_title("Invalid order")
        .with_instance("/orders/42/problems/7");

    let response = diagnostic_response
        .into_problem_details()
        .with_problem_policy(problem_policy);

A custom `type` URI should identify a documented problem type whose semantics
are part of the application's public HTTP contract. The URI should remain
stable for clients that use it to identify the problem category.

### `instance`, `request_id`, and `report_id`

These identifiers have different jobs and must not be treated as
interchangeable:

- `instance` is the RFC 9457 URI reference identifying this particular
  occurrence of the problem. `diagprint-axum` does not automatically populate
  it from another identifier.
- `request_id` identifies the HTTP request for request-level correlation. It
  may be shared by multiple diagnostics created while handling one request.
- `report_id` identifies one diagprint diagnostic report. It identifies the
  diagnostic, not the request as a whole.

For example, one HTTP request might have:

    request_id = req-01JEXAMPLE

while two separate diagnostics created during that request have:

    report_id = 550e8400-e29b-41d4-a716-446655440000
    report_id = 6ba7b810-9dad-11d1-80b4-00c04fd430c8

That separation allows request tracing and diagnostic correlation to remain
precise without overloading one identifier with multiple meanings.

A request ID can be attached to the Problem Details representation without
changing the diagnostic report ID:

    let response = diagnostic_response
        .into_problem_details()
        .with_request_id(request_id.to_string());

`request_id` and `report_id` are RFC 9457 extension members. They do not
replace the standard `instance` member.

### Problem Details privacy behavior

By default, Problem Details responses preserve the same fail-closed disclosure
rules as ordinary `DiagnosticResponse` JSON responses:

- internal diagnostic messages are redacted;
- diagnostic codes are redacted;
- internal attributes are not serialized;
- causes, notes, help, suggestions, and remediation data are not serialized;
- the report ID is included only when `ResponsePolicy` permits it;
- a supplied request ID can be omitted through `ProblemDetailsPolicy`;
- `Cache-Control: no-store` is preserved.

If an application deliberately exposes a diagnostic message or code through
`ResponsePolicy`, the Problem Details representation follows that same policy:

    let response_policy = ResponsePolicy::default()
        .expose_diagnostic_message(true)
        .expose_diagnostic_code(true);

    let response = DiagnosticResponse::new(status, diagnostic)
        .with_policy(response_policy)
        .into_problem_details();

Only enable that exposure when those values are intentionally safe and stable
parts of the public API.

### Root-level custom extension members

RFC 9457 allows problem types to define additional members directly at the
root of the Problem Details object. `diagprint-axum` supports these through
`with_extension`.

For example:

    let response = diagnostic_response
        .into_problem_details()
        .with_extension(
            "errors",
            serde_json::json!([
                {
                    "field": "email",
                    "code": "invalid_format"
                }
            ]),
        )?;

This produces a root-level member:

    {
      "type": "https://api.example.com/problems/validation",
      "title": "Validation failed",
      "status": 422,
      "detail": "The request contains invalid values.",
      "report_id": "...",
      "request_id": "...",
      "errors": [
        {
          "field": "email",
          "code": "invalid_format"
        }
      ]
    }

There is deliberately no nested `extensions` object. Unknown members remain
ordinary RFC 9457 extension members that standards-compliant clients can
ignore.

Extension names are validated. They must:

- be at least three ASCII characters long;
- begin with an ASCII letter;
- contain only ASCII letters, digits, and `_`.

The following names are reserved and cannot be replaced by custom extensions:

- `type`
- `title`
- `status`
- `detail`
- `instance`
- `report_id`
- `request_id`
- `code`

Duplicate extension keys are always rejected.

This applies across both public and internal extensions. A public extension
cannot silently replace an internal extension with the same name, and an
internal extension cannot silently replace a public one.

This behavior is intentional: duplicate keys are treated as construction
errors rather than using last-write-wins semantics.

### Internal extension members

Debugging state, internal validation details, or other implementation data can
be attached separately:

    let response = diagnostic_response
        .into_problem_details()
        .with_internal_extension(
            "validator_state",
            serde_json::json!({
                "rule": "email_format",
                "stage": 3
            }),
        )?;

Internal extensions are retained by the response object but are redacted from
HTTP output by default.

Exposure requires an explicit Problem Details policy:

    let problem_policy = ProblemDetailsPolicy::default()
        .expose_internal_extensions(true);

    let response = response.with_problem_policy(problem_policy);

Enabling this option creates an intentional externalization boundary. Internal
extension values should therefore be reviewed for secrets, credentials,
private user data, stack traces, implementation details, and other sensitive
state before exposure.

Public extensions are appropriate for stable machine-readable API data such as
validation errors, retry information, domain-specific state, or remediation
hints.

Internal extensions are appropriate for application-controlled diagnostic or
debugging state that should remain private unless deliberately exposed.

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

Creating a `DiagnosticResponse` or `ProblemDetailsResponse` does not
automatically:

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
- `ProblemDetails`
- `ProblemDetailsPolicy`
- `ProblemDetailsResponse`
- `ProblemDetailsResponseExt`

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

The complete companion-crate workflow is documented in:

    docs/COMPANION_CRATE_WORKFLOW.md

## Current scope

The v0.8 adapter currently provides:

- privacy-safe diagnostic HTTP responses;
- correlation through report IDs;
- explicit public-message/code policy;
- RFC 9457 Problem Details responses;
- custom problem types, titles, and instance URI references;
- separate request-ID and report-ID correlation;
- root-level RFC 9457 custom extension members;
- privacy-gated internal extension members;
- deterministic duplicate-extension rejection;
- JSON rejection diagnostics;
- path rejection diagnostics;
- query rejection diagnostics;
- form rejection diagnostics;
- required-extension rejection diagnostics.

Likely future work includes:

- DiagnosticReport responses;
- richer tracing/request context integration;
- application-error conversion;
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
