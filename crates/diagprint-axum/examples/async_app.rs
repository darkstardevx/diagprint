//! Minimal bounded asynchronous diagprint-axum application.
//!
//! Run from the workspace root with:
//!
//!     cargo run -p diagprint-axum \
//!         --example async_app \
//!         --features async-delivery

use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use diagprint::{Diagnostic, DiagnosticSink, Reporter, SinkResult};
use diagprint_axum::{
    ApplicationError, AsyncApplicationResultExt, AsyncDiagnosticSink, AsyncDiagnosticState,
    AsyncEmissionOutcome, AsyncEmittedProblemResult, BackpressurePolicy, REQUEST_ID_HEADER,
    RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use std::{error::Error, fmt, io, sync::Arc};
use tower::ServiceExt;

#[derive(Debug)]
struct ConsoleSink;

impl DiagnosticSink for ConsoleSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        eprintln!(
            "[async server diagnostic] report={} message={}",
            diagnostic.report_id, diagnostic.message,
        );

        Ok(())
    }
}

#[derive(Debug)]
struct OrderNotFound {
    id: u64,
}

impl fmt::Display for OrderNotFound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "internal async order lookup failed for {}",
            self.id,
        )
    }
}

impl Error for OrderNotFound {}

impl ApplicationError for OrderNotFound {
    fn http_status(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .warning(format!(
                "internal async order lookup failed for {}",
                self.id,
            ))
            .code("orders.async_not_found")
            .attribute("orders.internal_id", self.id.to_string())
    }
}

#[derive(Clone)]
struct AppState {
    diagnostics: AsyncDiagnosticState,
}

fn load_order(id: u64) -> Result<String, OrderNotFound> {
    if id == 7 {
        Ok("order 7 is ready".to_owned())
    } else {
        Err(OrderNotFound { id })
    }
}

async fn order_handler(
    State(state): State<AppState>,
    context: RequestContext,
    Path(id): Path<u64>,
) -> AsyncEmittedProblemResult<String> {
    load_order(id)
        .emit_problem_async(&state.diagnostics, &context)
        .await
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let reporter = Reporter::builder()
        .application("diagprint-axum-async-example")
        .build()?;

    let async_sink = Arc::new(AsyncDiagnosticSink::spawn(
        ConsoleSink,
        32,
        BackpressurePolicy::Block,
    )?);

    let state = AppState {
        diagnostics: AsyncDiagnosticState::new(reporter, Arc::clone(&async_sink)),
    };

    let response = {
        let app = Router::new()
            .route("/orders/{id}", get(order_handler))
            .route_layer(middleware::from_fn(request_context_middleware))
            .with_state(state);

        app.oneshot(
            Request::builder()
                .uri("/orders/42")
                .header(REQUEST_ID_HEADER, "example-async-request")
                .body(Body::empty())?,
        )
        .await
        .expect("Axum Router should be infallible")
    };

    let status = response.status();

    let request_id = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing>")
        .to_owned();

    let outcome = response.extensions().get::<AsyncEmissionOutcome>().cloned();

    // Enqueued means queue acceptance, not completed delivery.
    // Flush creates the completion boundary used by this example.
    async_sink.flush().await?;

    let body = response.into_body().collect().await?.to_bytes();

    println!("HTTP {status}");
    println!("x-request-id: {request_id}");
    println!("submission: {outcome:?}");
    println!("{}", String::from_utf8_lossy(&body));

    // The router and its application state have been dropped above, so the
    // example can recover the single sink owner and shut it down cleanly.
    let async_sink = Arc::try_unwrap(async_sink)
        .map_err(|_| io::Error::other("async diagnostic sink still has active owners"))?;

    async_sink.shutdown().await?;

    Ok(())
}
