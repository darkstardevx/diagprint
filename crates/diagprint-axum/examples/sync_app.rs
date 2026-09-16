//! Minimal synchronous diagprint-axum application.
//!
//! Run from the workspace root with:
//!
//!     cargo run -p diagprint-axum --example sync_app

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
    ApplicationError, ApplicationResultExt, DiagnosticState, EmittedProblemResult,
    REQUEST_ID_HEADER, RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use std::{error::Error, fmt, sync::Arc};
use tower::ServiceExt;

#[derive(Debug)]
struct ConsoleSink;

impl DiagnosticSink for ConsoleSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        eprintln!(
            "[server diagnostic] report={} message={}",
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
        write!(formatter, "internal order lookup failed for {}", self.id,)
    }
}

impl Error for OrderNotFound {}

impl ApplicationError for OrderNotFound {
    fn http_status(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .warning(format!("internal order lookup failed for {}", self.id,))
            .code("orders.not_found")
            .attribute("orders.internal_id", self.id.to_string())
    }
}

#[derive(Clone)]
struct AppState {
    diagnostics: DiagnosticState,
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
) -> EmittedProblemResult<String> {
    load_order(id).emit_problem(&state.diagnostics, &context)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let reporter = Reporter::builder()
        .application("diagprint-axum-sync-example")
        .build()?;

    let sink: Arc<dyn DiagnosticSink> = Arc::new(ConsoleSink);

    let state = AppState {
        diagnostics: DiagnosticState::new(reporter, sink),
    };

    let app = Router::new()
        .route("/orders/{id}", get(order_handler))
        .route_layer(middleware::from_fn(request_context_middleware))
        .with_state(state);

    // This in-memory request keeps the example runnable without binding a
    // network port. The same Router can be passed to axum::serve in a real
    // application.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/orders/42")
                .header(REQUEST_ID_HEADER, "example-sync-request")
                .body(Body::empty())?,
        )
        .await
        .expect("Axum Router should be infallible");

    let status = response.status();

    let request_id = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing>")
        .to_owned();

    let body = response.into_body().collect().await?.to_bytes();

    println!("HTTP {status}");
    println!("x-request-id: {request_id}");
    println!("{}", String::from_utf8_lossy(&body));

    Ok(())
}
