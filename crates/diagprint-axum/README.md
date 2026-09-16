# diagprint-axum

Privacy-safe Axum integration for the `diagprint` diagnostics lifecycle
framework.

`diagprint-axum` adapts structured diagnostics and Axum request failures into
HTTP responses without introducing Axum or an async runtime dependency into
the core `diagprint` crate.

## Quick start

The fastest way to understand the integration is to run the packaged examples.

Synchronous application state and explicit emission:

    cargo run -p diagprint-axum --example sync_app

Bounded asynchronous submission:

    cargo run -p diagprint-axum \
        --example async_app \
        --features async-delivery

Both examples use a real Axum `Router`, the first-class `RequestContext`
extractor, an `ApplicationError`, RFC 9457 Problem Details, and explicit
diagnostic delivery.

The examples deliberately use an in-memory request rather than binding a TCP
port, which keeps them deterministic and easy to run in CI. The resulting
`Router` is the same application value that can be passed to `axum::serve` in
a networked service.

The synchronous path uses `DiagnosticState`.

The asynchronous path uses `AsyncDiagnosticState` with one bounded
`AsyncDiagnosticSink`. Enabling `async-delivery` re-exports the common async
construction and lifecycle types from `diagprint-axum`, including:

- `AsyncDiagnosticSink`
- `AsyncSinkError`
- `BackpressurePolicy`
- `SubmitOutcome`
- `SubmissionSummary`
- `ReportSubmitError`

A separate direct `diagprint-async` dependency is therefore not required for
the common `diagprint-axum` async setup.

Queue ownership and lifecycle remain explicit. In particular,
`AsyncEmissionOutcome::Enqueued` means queue acceptance rather than completed
delivery.

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

For synchronous response integration:

    [dependencies]
    diagprint = "0.7"
    diagprint-axum = "0.8"

Asynchronous diagnostic delivery is optional:

    [dependencies]
    diagprint = "0.7"
    diagprint-axum = {
        version = "0.8",
        features = ["async-delivery"],
    }

The `async-delivery` feature adds the `diagprint-async` companion crate to the
Axum integration. It is disabled by default.

The common async construction and lifecycle types are re-exported directly
from `diagprint-axum`, so applications using the Axum integration do not need
to add a separate `diagprint-async` dependency merely to construct
`AsyncDiagnosticSink` or select a `BackpressurePolicy`.

A normal `diagprint-axum` dependency therefore does not pull the asynchronous
delivery layer into the application's normal dependency graph.

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

## Explicit diagnostic emission

Creating a `DiagnosticResponse`, adapting an application error, or rendering
RFC 9457 Problem Details remains side-effect free.

Applications that want to deliver the internal diagnostic to a diagprint sink
must opt in explicitly with `DiagnosticEmissionExt::emit_to`:

    use diagprint::DiagnosticSink;
    use diagprint_axum::{
        DiagnosticEmissionExt,
        DiagnosticResponse,
        EmissionOutcome,
    };

    let response = DiagnosticResponse::new(
        status,
        diagnostic,
    )
    .emit_to(&sink);

    match response.outcome() {
        EmissionOutcome::Emitted => {
            // The sink accepted the diagnostic.
        }
        EmissionOutcome::Failed(error) => {
            // The HTTP response is still available.
            eprintln!("diagnostic emission failed: {error}");
        }
    }

`emit_to` deliberately performs exactly one synchronous
`DiagnosticSink::emit` attempt.

It does not:

- emit automatically when a response is constructed;
- flush the sink;
- retry failed delivery;
- queue diagnostics;
- start background work;
- persist diagnostics unless the selected sink itself does so.

The sink receives the complete internal `Diagnostic`, not the privacy-filtered
HTTP representation. This allows server-side diagnostics to retain internal
messages, codes, structured attributes, causes, and other diagnostic context
while the client continues to receive the existing redacted response.

For response types, converting the returned emission wrapper into an Axum
response stores the `EmissionOutcome` in the response extensions.

Successful delivery is represented by:

    EmissionOutcome::Emitted

Sink failure is represented by:

    EmissionOutcome::Failed(error)

A sink failure does not replace the HTTP status, alter the response body, or
prevent the response from being returned.

The same explicit emission API works with RFC 9457 responses:

    let response = diagnostic_response
        .into_problem_details()
        .emit_to(&sink);

Emission and HTTP disclosure are therefore separate decisions:

- `DiagnosticSink` controls where the internal diagnostic is delivered;
- `ResponsePolicy` controls what diagnostic information may reach the client;
- `ProblemDetailsPolicy` controls the RFC 9457 representation.

No response or adapter emits diagnostics unless `emit_to` is called.

## Application-state ergonomics

Applications commonly need the same `Reporter` and diagnostic sink across many
handlers.

`DiagnosticState` packages those two values into one cloneable application
state component:

    use std::sync::Arc;
    use diagprint::Reporter;
    use diagprint_axum::DiagnosticState;

    let reporter = Reporter::builder()
        .application("api")
        .build()?;

    let diagnostics = DiagnosticState::new(
        reporter,
        Arc::new(sink),
    );

It can live alongside ordinary application state:

    #[derive(Clone)]
    struct AppState {
        diagnostics: DiagnosticState,
        orders: OrderService,
    }

A handler can then combine application state, the first-class
`RequestContext` extractor, domain error adaptation, RFC 9457 Problem Details,
and explicit sink delivery without manually passing a reporter and sink through
each function:

    use axum::{
        Json,
        extract::State,
    };

    async fn handler(
        State(state): State<AppState>,
        context: RequestContext,
    ) -> Result<Json<Order>, Emission<ProblemDetailsResponse>> {
        state
            .orders
            .load()
            .map(Json)
            .map_err(|error| {
                state
                    .diagnostics
                    .emit_problem(&error, &context)
            })
    }

`emit_problem` is deliberately named as a side effect.

It:

- creates the diagnostic through the application's `ApplicationError`;
- attaches privacy-safe request correlation;
- produces RFC 9457 Problem Details;
- performs exactly one synchronous `DiagnosticSink::emit` attempt;
- preserves the emission outcome as server-side response metadata.

It does not:

- emit successful responses;
- emit errors automatically;
- flush the sink;
- retry failures;
- create a queue;
- create background work;
- alter the client disclosure policy.

The sink receives the complete internal diagnostic. The HTTP client still
receives only the representation permitted by `ResponsePolicy` and
`ProblemDetailsPolicy`.

A sink failure remains `EmissionOutcome::Failed` and does not replace the
application's Problem Details response.

## Result ergonomics

Most applications already return domain-level `Result<T, E>` values from
service or repository functions.

When `E` implements `ApplicationError`, `ApplicationResultExt` converts only
the error side into correlated Problem Details and performs the explicit
synchronous diagnostic emission:

    fn load_order(id: u64) -> Result<Order, OrderError> {
        // domain or repository work
        # todo!()
    }

    async fn handler(
        State(state): State<AppState>,
        context: RequestContext,
        Path(id): Path<u64>,
    ) -> EmittedProblemResult<Order> {
        load_order(id)
            .emit_problem(
                &state.diagnostics,
                &context,
            )
    }

`Ok(T)` is returned unchanged. It does not emit a diagnostic.

`Err(E)` is converted through the same `DiagnosticState::emit_problem`
boundary used by the lower-level API. Sink failure remains observable through
`EmissionOutcome` and does not replace the Problem Details response.

With `async-delivery`, `AsyncApplicationResultExt` provides the bounded async
counterpart:

    async fn handler(
        State(state): State<AppState>,
        context: RequestContext,
        Path(id): Path<u64>,
    ) -> AsyncEmittedProblemResult<Order> {
        load_order(id)
            .emit_problem_async(
                &state.diagnostics,
                &context,
            )
            .await
    }

The async adapter preserves the existing submission meanings:

- `Enqueued` means queue acceptance, not completed delivery;
- `Dropped` means the configured backpressure policy deliberately dropped the
  diagnostic;
- `Failed` retains queue, worker, or submission failure.

The application error is converted into its correlated Problem Details response
before the returned future crosses the asynchronous submission boundary.
Consequently, an `ApplicationError` does not need to implement `Send` or
`Sync` merely to use `emit_problem_async`.

The successful `T` must be `Send`, because that value can be carried by the
returned handler future across an `.await`.

Neither adapter implements automatic logging or blanket `IntoResponse`
behavior for application errors. The call site still chooses explicitly
whether synchronous emission or bounded asynchronous submission occurs.

## Asynchronous diagnostic delivery

The optional `async-delivery` feature connects `diagprint-axum` to the bounded
delivery machinery provided by `diagprint-async`.

Enable it with:

    [dependencies]
    diagprint-axum = {
        version = "0.8",
        features = ["async-delivery"],
    }

The feature provides `AsyncDiagnosticEmissionExt::emit_to_async`:

    use diagprint_axum::{
        AsyncDiagnosticEmissionExt,
        AsyncEmissionOutcome,
        DiagnosticResponse,
    };

    let response = DiagnosticResponse::new(
        status,
        diagnostic,
    )
    .emit_to_async(&async_sink)
    .await;

    match response.outcome() {
        AsyncEmissionOutcome::Enqueued => {
            // Accepted into the bounded delivery queue.
        }

        AsyncEmissionOutcome::Dropped => {
            // Explicitly dropped by the configured backpressure policy.
        }

        AsyncEmissionOutcome::Failed(error) => {
            // Submission failed, but the HTTP response is still available.
            eprintln!("diagnostic submission failed: {error}");
        }
    }

### Async application state

With `async-delivery` enabled, `AsyncDiagnosticState` provides the asynchronous
counterpart to `DiagnosticState`.

It stores one `Reporter` together with a shared `Arc<AsyncDiagnosticSink>`:

    let async_diagnostics = AsyncDiagnosticState::new(
        reporter,
        Arc::clone(&async_sink),
    );

The state can live beside the rest of an Axum application's resources:

    #[derive(Clone)]
    struct AppState {
        diagnostics: AsyncDiagnosticState,
        orders: OrderService,
    }

A handler can combine `State`, `RequestContext`, domain error adaptation,
Problem Details, and bounded async submission:

    async fn handler(
        State(state): State<AppState>,
        context: RequestContext,
    ) -> Result<Json<Order>, AsyncEmission<ProblemDetailsResponse>> {
        match state.orders.load() {
            Ok(order) => Ok(Json(order)),
            Err(error) => Err(
                state
                    .diagnostics
                    .emit_problem(&error, &context)
                    .await,
            ),
        }
    }

`emit_problem(...).await` is an explicit submission operation.

It does not:

- create another queue;
- spawn one worker per request;
- retry failed submissions;
- flush automatically;
- shut down the async sink.

`AsyncEmissionOutcome::Enqueued` still means queue acceptance only.

`AsyncEmissionOutcome::Dropped` still represents an explicit `DropNewest`
backpressure decision.

Queue rejection remains `AsyncEmissionOutcome::Failed`.

The application owns the async sink lifecycle. A typical application keeps an
external shared handle while the router is running, drops router/application
state during graceful shutdown, and then recovers ownership or otherwise
arranges for `AsyncDiagnosticSink::shutdown` at the lifecycle boundary.

### Submission is not delivery completion

`AsyncEmissionOutcome::Enqueued` means the diagnostic was accepted into the
bounded asynchronous delivery queue.

It does not mean that:

- the underlying sink has already written the diagnostic;
- buffered output has been flushed;
- persistence has completed;
- application shutdown may discard the delivery worker.

Applications that require completion guarantees should use the lifecycle
operations provided by `diagprint-async`, such as `flush` or `shutdown`, at the
appropriate application lifecycle boundary.

### Backpressure remains explicit

`diagprint-axum` does not define a second queue or a second set of overload
rules.

Queue capacity and overload behavior come directly from
`diagprint-async::AsyncDiagnosticSink`.

The supported backpressure policies include:

- `Block` — wait asynchronously for queue capacity;
- `Reject` — reject a diagnostic when the bounded queue is full;
- `DropNewest` — explicitly permit selected severities to be dropped.

The corresponding Axum submission result remains observable through
`AsyncEmissionOutcome`.

A queue rejection becomes `AsyncEmissionOutcome::Failed`.

An intentional `DropNewest` decision becomes
`AsyncEmissionOutcome::Dropped`.

Neither case silently becomes `Enqueued`.

### One shared sink, not one worker per request

An `AsyncDiagnosticSink` should normally be created as application-level state
and reused across requests.

For example, an application can construct one bounded sink during startup,
place access to it in application state, submit diagnostics from handlers, and
shut it down during graceful application shutdown.

`emit_to_async` itself does not:

- create an additional queue;
- spawn a Tokio task for each HTTP request;
- create a delivery worker for each diagnostic;
- flush after each diagnostic;
- retry failed delivery;
- shut down the sink.

Worker ownership remains explicit at the application lifecycle level.

### HTTP behavior remains independent

As with synchronous `emit_to`, asynchronous submission does not widen the HTTP
privacy boundary.

The async sink receives the complete internal diagnostic.

The HTTP client still receives only the representation permitted by
`ResponsePolicy` and `ProblemDetailsPolicy`.

Submission failure, queue rejection, or an intentional drop does not replace
the HTTP response.

For values converted into an Axum response, `AsyncEmissionOutcome` is stored in
response extensions as server-side metadata. It is not serialized into the
client response body.

### Feature isolation

Asynchronous delivery is intentionally optional.

Without `async-delivery`:

- `AsyncDiagnosticEmissionExt` is not compiled;
- `AsyncEmission` is not compiled;
- `AsyncEmissionOutcome` is not compiled;
- `diagprint-async` is not a normal dependency of `diagprint-axum`;
- synchronous response adaptation and explicit `emit_to` continue to work
  independently.

This keeps applications that only want privacy-safe Axum response adaptation
from paying for an asynchronous delivery layer they do not use.

## Request context and correlation

`diagprint-axum` provides request-context middleware for correlating HTTP
requests with internal diagnostics without capturing sensitive request data.

Apply the middleware after declaring routes:

    use axum::{Router, middleware, routing::get};
    use diagprint_axum::request_context_middleware;

    let app = Router::new()
        .route("/users/{id}", get(handler))
        .route_layer(
            middleware::from_fn(request_context_middleware)
        );

Handlers can extract the established request context directly:

    use axum::Json;
    use diagprint_axum::RequestContext;

    async fn handler(context: RequestContext) -> Json<String> {
        Json(context.request_id().to_string())
    }

`RequestContext` is a first-class Axum extractor. Applications do not need to
spell `Extension<RequestContext>` or manually read request extensions.

If a handler requests `RequestContext` without installing the correlation
middleware, extraction fails closed with HTTP 500. The rejection does not
expose the application's middleware configuration to the client.

The middleware:

- preserves a valid inbound `x-request-id`;
- generates a UUIDv7 request ID when the header is missing or invalid;
- stores a `RequestContext` in request extensions;
- records the HTTP method;
- records Axum's matched route pattern when available;
- propagates the selected request ID through the response header.

A handler can retrieve the context with Axum's `Extension` extractor:

    use axum::Extension;
    use diagprint_axum::RequestContext;

    async fn handler(
        Extension(context): Extension<RequestContext>,
    ) {
        let request_id = context.request_id();
    }

Request context deliberately does not retain:

- raw request paths;
- query strings;
- request bodies;
- cookies;
- authorization headers;
- arbitrary request headers.

This means a route such as:

    /users/42?token=secret

can be represented internally as the matched route:

    /users/{id}

without retaining either the concrete user identifier or query-string secret.

### Request IDs and report IDs

Request IDs and diagprint report IDs represent different identities.

A request ID correlates one HTTP request.

A report ID identifies one structured diagnostic.

One HTTP request can therefore produce multiple diagnostics with distinct
report IDs while all share the same request ID.

### Application errors with request context

Application-error adapters can attach request context directly:

    let response = error
        .to_problem_response_with_context(
            &reporter,
            &request_context,
        );

This adds safe request attributes to the internal diagnostic:

    http.request_id
    http.method
    http.route

and includes the request ID in the RFC 9457 response while keeping the
diagnostic report ID distinct.

Request-context adaptation does not emit, log, or persist diagnostics.

## Opt-in diagnostic emission

HTTP response construction and diagnostic emission are deliberately separate.

Creating a `DiagnosticResponse` or `ProblemDetailsResponse` does not emit,
log, persist, flush, or otherwise forward its internal diagnostic.

Emission happens only when the application explicitly calls `emit_to`:

    use diagprint_axum::DiagnosticEmissionExt;

    let emitted = error
        .to_problem_response_with_context(
            &reporter,
            &request_context,
        )
        .emit_to(&diagnostic_sink);

`emit_to` accepts any `diagprint::DiagnosticSink`.

The sink receives the full internal diagnostic, including safe request
correlation attributes such as:

    http.request_id
    http.method
    http.route

Client disclosure remains controlled independently by `ResponsePolicy` and
`ProblemDetailsPolicy`.

### Emission failure

Sink failure does not replace or prevent the HTTP response.

Instead, `emit_to` returns an `Emission<T>` containing:

- the original response;
- an `EmissionOutcome`.

The outcome is either:

    EmissionOutcome::Emitted

or:

    EmissionOutcome::Failed(error)

`Emission<T>` implements Axum's `IntoResponse` whenever `T` does, so a failed
diagnostic sink cannot turn an application error response into a different
HTTP failure.

When converted into an Axum response, the `EmissionOutcome` is retained in
the response extensions for server-side middleware inspection. It is not
serialized into the client body.

### Lifecycle ownership

`emit_to` performs exactly one `DiagnosticSink::emit` call.

It does not automatically call `flush`.

Persistence, buffering, flushing, retrying, queueing, and asynchronous
delivery remain responsibilities of the selected sink and the application.

## Application error adapters

Application and domain errors can implement `ApplicationError` to define one
explicit mapping from domain semantics into diagprint diagnostics and HTTP
responses.

The mapping controls:

- HTTP status;
- internal diagnostic construction;
- client disclosure policy;
- RFC 9457 problem type and title.

For example:

    use axum::http::StatusCode;
    use diagprint::{Diagnostic, Reporter};
    use diagprint_axum::{
        ApplicationError,
        ApplicationErrorExt,
        ProblemDetailsPolicy,
    };
    use std::{error::Error, fmt};

    #[derive(Debug)]
    struct OrderNotFound;

    impl fmt::Display for OrderNotFound {
        fn fmt(
            &self,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            write!(f, "internal order lookup failed")
        }
    }

    impl Error for OrderNotFound {}

    impl ApplicationError for OrderNotFound {
        fn http_status(&self) -> StatusCode {
            StatusCode::NOT_FOUND
        }

        fn to_diagnostic(
            &self,
            reporter: &Reporter,
        ) -> Diagnostic {
            reporter
                .warning("order was not found")
                .code("shop.order.not_found")
        }

        fn problem_policy(&self) -> ProblemDetailsPolicy {
            ProblemDetailsPolicy::default()
                .with_type_uri(
                    "https://api.example.com/problems/order-not-found",
                )
                .with_title("Order not found")
        }
    }

The resulting adapter call is small:

    let response =
        error.to_problem_response(&reporter);

The application can also request the existing JSON diagnostic response:

    let response =
        error.to_diagnostic_response(&reporter);

Application error adaptation is deliberately explicit.

`diagprint-axum` does not infer HTTP statuses from diagnostic severity, and it
does not automatically expose an error's `Display` representation to the
client.

This prevents internal error strings from accidentally becoming part of the
public API.

### Application errors and Problem Details extensions

Public and internal RFC 9457 extensions compose with application-error
responses after adaptation:

    let response = error
        .to_problem_response(&reporter)
        .with_extension(
            "errors",
            serde_json::json!([
                {
                    "field": "email",
                    "code": "invalid_format"
                }
            ]),
        )?;

The existing extension validation rules still apply:

- reserved names are rejected;
- invalid names are rejected;
- duplicate names are rejected;
- internal extensions remain private by default.

Request correlation also layers normally:

    let response = error
        .to_problem_response(&reporter)
        .with_request_id(request_id.to_string());

This keeps domain-error mapping, diagnostic identity, request correlation, and
HTTP representation separate but composable.

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
- explicit application/domain error adapters;
- JSON rejection diagnostics;
- path rejection diagnostics;
- query rejection diagnostics;
- form rejection diagnostics;
- required-extension rejection diagnostics.

Likely future work includes:

- DiagnosticReport responses;
- richer tracing/request context integration;
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
