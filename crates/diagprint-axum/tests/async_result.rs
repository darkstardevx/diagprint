#![cfg(feature = "async-delivery")]

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

#[derive(Clone, Default)]
struct RecordingSink {
    observed: Arc<Mutex<Vec<ObservedDiagnostic>>>,
    emit_count: Arc<AtomicUsize>,
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
        .application("diagprint-axum-async-result-test")
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
            "private async result lookup failure for order {}",
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
                "private async result lookup failure for order {}",
                self.id,
            ))
            .code("orders.async_result_not_found")
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
    diagnostics: AsyncDiagnosticState,
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

fn application(sink: Arc<AsyncDiagnosticSink>) -> Router {
    let state = AppState {
        diagnostics: AsyncDiagnosticState::new(reporter(), sink),
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

async fn shutdown_shared(sink: Arc<AsyncDiagnosticSink>) {
    let sink = Arc::try_unwrap(sink).unwrap_or_else(|sink| {
        panic!(
            "async sink still has {} strong references",
            Arc::strong_count(&sink)
        );
    });

    sink.shutdown()
        .await
        .expect("async diagnostic sink should shut down cleanly");
}

#[tokio::test]
async fn ok_async_result_passes_through_without_submission() {
    let recording = RecordingSink::default();

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(recording.clone(), 8, BackpressurePolicy::Block)
            .expect("async sink should spawn"),
    );

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

    assert!(
        response
            .extensions()
            .get::<AsyncEmissionOutcome>()
            .is_none()
    );

    sink.flush().await.expect("async sink should flush");

    assert_eq!(recording.emit_count(), 0);
    assert!(recording.observed().is_empty());

    shutdown_shared(sink).await;
}

#[tokio::test]
async fn error_async_result_is_enqueued_correlated_and_redacted() {
    let recording = RecordingSink::default();

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(recording.clone(), 8, BackpressurePolicy::Block)
            .expect("async sink should spawn"),
    );

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/42?token=top-secret")
                .header(REQUEST_ID_HEADER, "request-async-result-42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        response.extensions().get::<AsyncEmissionOutcome>(),
        Some(&AsyncEmissionOutcome::Enqueued)
    );

    // Queue acceptance is not completed delivery.
    sink.flush()
        .await
        .expect("queued diagnostic should be delivered");

    assert_eq!(recording.emit_count(), 1);

    let observed = recording.observed();

    assert_eq!(observed.len(), 1);

    assert_eq!(
        observed[0].message,
        "private async result lookup failure for order 42"
    );

    assert_eq!(
        observed[0].code.as_deref(),
        Some("orders.async_result_not_found")
    );

    let body = body_json(response).await;

    assert_eq!(body["status"], 404);

    assert_eq!(body["request_id"], "request-async-result-42");

    let serialized = serde_json::to_string(&body).expect("problem response should serialize");

    for forbidden in [
        "private async result lookup failure",
        "orders.async_result_not_found",
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

    shutdown_shared(sink).await;
}

#[derive(Debug)]
struct LocalOnlyError {
    _marker: std::rc::Rc<()>,
}

impl fmt::Display for LocalOnlyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("private local-only application error")
    }
}

impl Error for LocalOnlyError {}

impl ApplicationError for LocalOnlyError {
    fn http_status(&self) -> StatusCode {
        StatusCode::UNPROCESSABLE_ENTITY
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .warning("private local-only application error")
            .code("local.only.error")
    }
}

#[tokio::test]
async fn application_error_does_not_need_send_or_sync_for_async_adapter() {
    fn assert_send<T: Send>(value: T) -> T {
        value
    }

    let recording = RecordingSink::default();

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(recording.clone(), 8, BackpressurePolicy::Block)
            .expect("async sink should spawn"),
    );

    let state = AsyncDiagnosticState::new(reporter(), Arc::clone(&sink));

    let context = RequestContext::new(
        diagprint_axum::RequestId::new("request-local-only").expect("request ID should be valid"),
        axum::http::Method::POST,
        Some("/local-only".to_owned()),
    );

    // Rc makes LocalOnlyError neither Send nor Sync.
    //
    // The adapter must still produce a Send future because the error is
    // converted into ProblemDetailsResponse before the async boundary.
    let future = Result::<(), LocalOnlyError>::Err(LocalOnlyError {
        _marker: std::rc::Rc::new(()),
    })
    .emit_problem_async(&state, &context);

    let adapted = assert_send(future).await;

    let emission = match adapted {
        Ok(()) => {
            panic!("local-only error unexpectedly became success");
        }

        Err(emission) => emission,
    };

    assert_eq!(emission.outcome(), &AsyncEmissionOutcome::Enqueued);

    // Enqueued is queue acceptance, not delivery completion.
    sink.flush()
        .await
        .expect("local-only diagnostic should flush");

    assert_eq!(recording.emit_count(), 1);

    let observed = recording.observed();

    assert_eq!(observed.len(), 1);

    assert_eq!(observed[0].message, "private local-only application error");

    assert_eq!(observed[0].code.as_deref(), Some("local.only.error"));

    drop(state);

    shutdown_shared(sink).await;
}
