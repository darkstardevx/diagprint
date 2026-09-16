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
    ApplicationError, ApplicationResultExt, DiagnosticState, EmissionOutcome, EmittedProblemResult,
    REQUEST_ID_HEADER, RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedDiagnostic {
    message: String,
    code: Option<String>,
}

#[derive(Default)]
struct RecordingSink {
    observed: Mutex<Vec<ObservedDiagnostic>>,
    emit_count: AtomicUsize,
}

impl RecordingSink {
    fn emit_count(&self) -> usize {
        self.emit_count.load(Ordering::SeqCst)
    }

    fn observed(&self) -> Vec<ObservedDiagnostic> {
        self.observed
            .lock()
            .expect("recording sink lock should not be poisoned")
            .clone()
    }
}

impl DiagnosticSink for RecordingSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        self.emit_count.fetch_add(1, Ordering::SeqCst);

        self.observed
            .lock()
            .expect("recording sink lock should not be poisoned")
            .push(ObservedDiagnostic {
                message: diagnostic.message.clone(),
                code: diagnostic.code.clone(),
            });

        Ok(())
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-result-test")
        .build()
        .expect("test reporter should build")
}

#[derive(Debug)]
struct OrderNotFound {
    id: u64,
}

impl fmt::Display for OrderNotFound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "private result lookup failure for order {}",
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
                "private result lookup failure for order {}",
                self.id,
            ))
            .code("orders.result_not_found")
            .attribute("orders.internal_id", self.id.to_string())
    }
}

fn load_order(id: u64) -> Result<String, OrderNotFound> {
    if id == 7 {
        Ok("order 7 is ready".to_owned())
    } else {
        Err(OrderNotFound { id })
    }
}

#[derive(Clone)]
struct AppState {
    diagnostics: DiagnosticState,
}

async fn order_handler(
    State(state): State<AppState>,
    context: RequestContext,
    Path(id): Path<u64>,
) -> EmittedProblemResult<String> {
    load_order(id).emit_problem(&state.diagnostics, &context)
}

fn application(sink: Arc<RecordingSink>) -> Router {
    let diagnostic_sink: Arc<dyn DiagnosticSink> = sink;

    let state = AppState {
        diagnostics: DiagnosticState::new(reporter(), diagnostic_sink),
    };

    Router::new()
        .route("/orders/{id}", get(order_handler))
        .route_layer(middleware::from_fn(request_context_middleware))
        .with_state(state)
}

async fn body_json(response: axum::response::Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    serde_json::from_slice(&body).expect("response should contain JSON")
}

#[tokio::test]
async fn ok_result_passes_through_without_emission() {
    let sink = Arc::new(RecordingSink::default());

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/7")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(sink.emit_count(), 0);
    assert!(sink.observed().is_empty());

    assert!(response.extensions().get::<EmissionOutcome>().is_none());

    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    assert_eq!(String::from_utf8_lossy(&body), "order 7 is ready");
}

#[tokio::test]
async fn error_result_is_correlated_emitted_and_redacted() {
    let sink = Arc::new(RecordingSink::default());

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/42?token=top-secret")
                .header(REQUEST_ID_HEADER, "request-result-42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        response.extensions().get::<EmissionOutcome>(),
        Some(&EmissionOutcome::Emitted)
    );

    assert_eq!(sink.emit_count(), 1);

    let observed = sink.observed();

    assert_eq!(observed.len(), 1);

    assert_eq!(
        observed[0].message,
        "private result lookup failure for order 42"
    );

    assert_eq!(observed[0].code.as_deref(), Some("orders.result_not_found"));

    let body = body_json(response).await;

    assert_eq!(body["status"], 404);

    assert_eq!(body["request_id"], "request-result-42");

    let serialized = serde_json::to_string(&body).expect("problem response should serialize");

    for forbidden in [
        "private result lookup failure",
        "orders.result_not_found",
        "orders.internal_id",
        "/orders/42",
        "top-secret",
        "token=",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "client response leaked forbidden value: {forbidden}"
        );
    }
}
