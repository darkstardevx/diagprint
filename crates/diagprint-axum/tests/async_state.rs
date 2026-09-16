#![cfg(feature = "async-delivery")]

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use diagprint::{Diagnostic, DiagnosticSink, DiagnosticValue, Reporter, Severity, SinkResult};
use diagprint_async::{AsyncDiagnosticSink, AsyncSinkError, BackpressurePolicy, SubmitOutcome};
use diagprint_axum::{
    ApplicationError, AsyncDiagnosticState, AsyncEmission, AsyncEmissionOutcome,
    ProblemDetailsResponse, REQUEST_ID_HEADER, RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    error::Error,
    fmt,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tower::ServiceExt;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedDiagnostic {
    message: String,
    code: Option<String>,
    request_id: Option<String>,
    method: Option<String>,
    route: Option<String>,
}

#[derive(Clone, Default)]
struct RecordingSink {
    observed: Arc<Mutex<Vec<ObservedDiagnostic>>>,
}

impl RecordingSink {
    fn observed(&self) -> Vec<ObservedDiagnostic> {
        self.observed
            .lock()
            .expect("recording sink lock should not be poisoned")
            .clone()
    }
}

impl DiagnosticSink for RecordingSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        self.observed
            .lock()
            .expect("recording sink lock should not be poisoned")
            .push(ObservedDiagnostic {
                message: diagnostic.message.clone(),
                code: diagnostic.code.clone(),
                request_id: string_attribute(diagnostic, "http.request_id"),
                method: string_attribute(diagnostic, "http.method"),
                route: string_attribute(diagnostic, "http.route"),
            });

        Ok(())
    }
}

fn string_attribute(diagnostic: &Diagnostic, name: &str) -> Option<String> {
    diagnostic
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .and_then(|attribute| match &attribute.value {
            DiagnosticValue::String(value) => Some(value.clone()),
            _ => None,
        })
}

#[derive(Default)]
struct BlockingState {
    entered: Mutex<usize>,
    entered_changed: Condvar,
    released: Mutex<bool>,
    released_changed: Condvar,
}

impl BlockingState {
    fn wait_until_entered(&self, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);

        let mut entered = self
            .entered
            .lock()
            .expect("blocking sink entered lock should not be poisoned");

        while *entered < expected {
            let now = Instant::now();

            assert!(now < deadline, "timed out waiting for blocking sink");

            let remaining = deadline.saturating_duration_since(now);

            let (guard, timeout) = self
                .entered_changed
                .wait_timeout(entered, remaining)
                .expect("blocking sink condvar should not be poisoned");

            entered = guard;

            assert!(
                !timeout.timed_out() || *entered >= expected,
                "timed out waiting for blocking sink"
            );
        }
    }

    fn release(&self) {
        let mut released = self
            .released
            .lock()
            .expect("blocking sink release lock should not be poisoned");

        *released = true;
        self.released_changed.notify_all();
    }
}

#[derive(Clone)]
struct BlockingSink {
    state: Arc<BlockingState>,
}

impl DiagnosticSink for BlockingSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        {
            let mut entered = self
                .state
                .entered
                .lock()
                .expect("blocking sink entered lock should not be poisoned");

            *entered += 1;
            self.state.entered_changed.notify_all();
        }

        let mut released = self
            .state
            .released
            .lock()
            .expect("blocking sink release lock should not be poisoned");

        while !*released {
            released = self
                .state
                .released_changed
                .wait(released)
                .expect("blocking sink condvar should not be poisoned");
        }

        Ok(())
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-async-state-test")
        .build()
        .expect("test reporter should build")
}

#[derive(Debug)]
struct OrderLookupError {
    order_id: u64,
}

impl fmt::Display for OrderLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "private asynchronous lookup failure for order {}",
            self.order_id
        )
    }
}

impl Error for OrderLookupError {}

impl ApplicationError for OrderLookupError {
    fn http_status(&self) -> StatusCode {
        StatusCode::NOT_FOUND
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .warning(format!(
                "private asynchronous lookup failure for order {}",
                self.order_id
            ))
            .code("orders.private_async_not_found")
            .attribute("orders.internal_id", self.order_id.to_string())
    }
}

#[derive(Clone, Default)]
struct OrderService;

impl OrderService {
    fn find(&self, order_id: u64) -> Result<Value, OrderLookupError> {
        if order_id == 7 {
            Ok(json!({
                "id": 7,
                "status": "ready",
            }))
        } else {
            Err(OrderLookupError { order_id })
        }
    }
}

#[derive(Clone)]
struct AppState {
    diagnostics: AsyncDiagnosticState,
    orders: OrderService,
}

async fn order_handler(
    State(state): State<AppState>,
    context: RequestContext,
    Path(order_id): Path<u64>,
) -> Result<Json<Value>, AsyncEmission<ProblemDetailsResponse>> {
    match state.orders.find(order_id) {
        Ok(order) => Ok(Json(order)),

        Err(error) => Err(state.diagnostics.emit_problem(&error, &context).await),
    }
}

fn application(sink: Arc<AsyncDiagnosticSink>) -> Router {
    let state = AppState {
        diagnostics: AsyncDiagnosticState::new(reporter(), sink),
        orders: OrderService,
    };

    Router::new()
        .route("/orders/{id}", get(order_handler))
        .route_layer(middleware::from_fn(request_context_middleware))
        .with_state(state)
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    serde_json::from_slice(&bytes).expect("response should contain valid JSON")
}

fn direct_warning(label: &str) -> Diagnostic {
    reporter()
        .warning(format!("queue prefill {label}"))
        .code("queue.prefill")
}

async fn shutdown_shared(sink: Arc<AsyncDiagnosticSink>) {
    match Arc::try_unwrap(sink) {
        Ok(sink) => {
            sink.shutdown()
                .await
                .expect("async diagnostic sink should shut down cleanly");
        }

        Err(sink) => {
            panic!(
                "async sink still has {} strong references at shutdown",
                Arc::strong_count(&sink)
            );
        }
    }
}

#[tokio::test]
async fn async_state_submits_problem_and_preserves_client_privacy() {
    let recording = RecordingSink::default();

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(recording.clone(), 8, BackpressurePolicy::Block)
            .expect("async diagnostic sink should spawn"),
    );

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/42?token=top-secret")
                .header(REQUEST_ID_HEADER, "request-async-state-42")
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

    sink.flush().await.expect("queued diagnostic should flush");

    let observed = recording.observed();

    assert_eq!(observed.len(), 1);

    assert_eq!(
        observed[0].message,
        "private asynchronous lookup failure for order 42"
    );

    assert_eq!(
        observed[0].code.as_deref(),
        Some("orders.private_async_not_found")
    );

    assert_eq!(
        observed[0].request_id.as_deref(),
        Some("request-async-state-42")
    );

    assert_eq!(observed[0].method.as_deref(), Some("GET"));

    assert_eq!(observed[0].route.as_deref(), Some("/orders/{id}"));

    let problem = body_json(response).await;

    let serialized = serde_json::to_string(&problem).expect("problem response should serialize");

    assert_eq!(problem["status"], 404);

    assert_eq!(problem["request_id"], "request-async-state-42");

    for forbidden in [
        "private asynchronous lookup failure",
        "orders.private_async_not_found",
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

#[tokio::test]
async fn successful_async_state_path_submits_nothing() {
    let recording = RecordingSink::default();

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(recording.clone(), 8, BackpressurePolicy::Block)
            .expect("async diagnostic sink should spawn"),
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

    sink.flush().await.expect("empty async sink should flush");

    assert!(recording.observed().is_empty());

    shutdown_shared(sink).await;
}

#[tokio::test]
async fn reject_backpressure_is_preserved_through_real_router() {
    let blocking = Arc::new(BlockingState::default());

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(
            BlockingSink {
                state: Arc::clone(&blocking),
            },
            1,
            BackpressurePolicy::Reject,
        )
        .expect("async diagnostic sink should spawn"),
    );

    assert_eq!(
        sink.emit(direct_warning("worker"),).await,
        Ok(SubmitOutcome::Enqueued)
    );

    blocking.wait_until_entered(1);

    assert_eq!(
        sink.emit(direct_warning("queued"),).await,
        Ok(SubmitOutcome::Enqueued)
    );

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/42")
                .header(REQUEST_ID_HEADER, "request-reject-42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let outcome = response
        .extensions()
        .get::<AsyncEmissionOutcome>()
        .expect("async outcome should be retained");

    match outcome {
        AsyncEmissionOutcome::Failed(AsyncSinkError::QueueFull { severity }) => {
            assert_eq!(*severity, Severity::Warning);
        }

        other => {
            panic!("expected queue-full outcome, got {other:?}");
        }
    }

    let problem = body_json(response).await;

    assert_eq!(problem["request_id"], "request-reject-42");

    blocking.release();

    shutdown_shared(sink).await;
}

#[tokio::test]
async fn drop_newest_is_preserved_through_real_router() {
    let blocking = Arc::new(BlockingState::default());

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(
            BlockingSink {
                state: Arc::clone(&blocking),
            },
            1,
            BackpressurePolicy::DropNewest {
                up_to: Severity::Warning,
            },
        )
        .expect("async diagnostic sink should spawn"),
    );

    assert_eq!(
        sink.emit(direct_warning("worker"),).await,
        Ok(SubmitOutcome::Enqueued)
    );

    blocking.wait_until_entered(1);

    assert_eq!(
        sink.emit(direct_warning("queued"),).await,
        Ok(SubmitOutcome::Enqueued)
    );

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/99")
                .header(REQUEST_ID_HEADER, "request-drop-99")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        response.extensions().get::<AsyncEmissionOutcome>(),
        Some(&AsyncEmissionOutcome::Dropped)
    );

    let problem = body_json(response).await;

    assert_eq!(problem["request_id"], "request-drop-99");

    blocking.release();

    shutdown_shared(sink).await;
}

#[tokio::test]
async fn async_state_clones_share_one_queue_and_reporter_session() {
    let recording = RecordingSink::default();

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(recording, 8, BackpressurePolicy::Block)
            .expect("async diagnostic sink should spawn"),
    );

    let state = AsyncDiagnosticState::new(reporter(), Arc::clone(&sink));

    let cloned = state.clone();

    assert_eq!(
        state.reporter().session_id(),
        cloned.reporter().session_id()
    );

    assert!(Arc::ptr_eq(&state.shared_sink(), &cloned.shared_sink(),));

    assert_eq!(state.sink().max_capacity(), cloned.sink().max_capacity());

    drop(cloned);
    drop(state);

    shutdown_shared(sink).await;
}
