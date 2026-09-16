use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware,
    routing::get,
};
use diagprint::{
    Diagnostic, DiagnosticSink, DiagnosticValue, Reporter, SinkError, SinkErrorKind, SinkResult,
};
use diagprint_axum::{
    ApplicationError, DiagnosticState, Emission, EmissionOutcome, ProblemDetailsResponse,
    REQUEST_ID_HEADER, RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
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

#[derive(Default)]
struct RecordingSink {
    observed: Mutex<Vec<ObservedDiagnostic>>,
    emit_count: AtomicUsize,
    flush_count: AtomicUsize,
    fail: AtomicBool,
}

impl RecordingSink {
    fn failing() -> Self {
        Self {
            fail: AtomicBool::new(true),
            ..Self::default()
        }
    }

    fn emit_count(&self) -> usize {
        self.emit_count.load(Ordering::SeqCst)
    }

    fn flush_count(&self) -> usize {
        self.flush_count.load(Ordering::SeqCst)
    }

    fn observed(&self) -> Vec<ObservedDiagnostic> {
        self.observed
            .lock()
            .expect("test sink lock should not be poisoned")
            .clone()
    }
}

impl DiagnosticSink for RecordingSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        self.emit_count.fetch_add(1, Ordering::SeqCst);

        if self.fail.load(Ordering::SeqCst) {
            return Err(SinkError::new(
                SinkErrorKind::Io,
                "simulated application-state sink failure",
            ));
        }

        self.observed
            .lock()
            .expect("test sink lock should not be poisoned")
            .push(ObservedDiagnostic {
                message: diagnostic.message.clone(),
                code: diagnostic.code.clone(),
                request_id: string_attribute(diagnostic, "http.request_id"),
                method: string_attribute(diagnostic, "http.method"),
                route: string_attribute(diagnostic, "http.route"),
            });

        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        self.flush_count.fetch_add(1, Ordering::SeqCst);
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

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-state-test")
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
            "private database lookup failure for order {}",
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
                "private database lookup failure for order {}",
                self.order_id
            ))
            .code("orders.private_not_found")
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
    diagnostics: DiagnosticState,
    orders: OrderService,
}

async fn order_handler(
    State(state): State<AppState>,
    context: RequestContext,
    Path(order_id): Path<u64>,
) -> Result<Json<Value>, Emission<ProblemDetailsResponse>> {
    state
        .orders
        .find(order_id)
        .map(Json)
        .map_err(|error| state.diagnostics.emit_problem(&error, &context))
}

fn application(sink: Arc<RecordingSink>) -> Router {
    let state = AppState {
        diagnostics: DiagnosticState::new(reporter(), sink),
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

    serde_json::from_slice(&bytes).expect("response body should contain valid JSON")
}

#[tokio::test]
async fn app_state_combines_domain_state_context_problem_details_and_emission() {
    let sink = Arc::new(RecordingSink::default());

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/42?token=top-secret")
                .header(REQUEST_ID_HEADER, "request-state-42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("request-state-42")
    );

    assert_eq!(
        response.extensions().get::<EmissionOutcome>(),
        Some(&EmissionOutcome::Emitted)
    );

    assert_eq!(sink.emit_count(), 1);
    assert_eq!(sink.flush_count(), 0);

    let observed = sink.observed();

    assert_eq!(observed.len(), 1);

    assert_eq!(
        observed[0].message,
        "private database lookup failure for order 42"
    );

    assert_eq!(
        observed[0].code.as_deref(),
        Some("orders.private_not_found")
    );

    assert_eq!(observed[0].request_id.as_deref(), Some("request-state-42"));

    assert_eq!(observed[0].method.as_deref(), Some("GET"));

    assert_eq!(observed[0].route.as_deref(), Some("/orders/{id}"));

    let problem = body_json(response).await;
    let serialized = serde_json::to_string(&problem).expect("problem response should serialize");

    assert_eq!(problem["status"], 404);
    assert_eq!(problem["request_id"], "request-state-42");

    assert_eq!(problem["detail"], "Request could not be completed.");

    for forbidden in [
        "private database lookup failure",
        "orders.private_not_found",
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

#[tokio::test]
async fn successful_application_state_path_does_not_emit() {
    let sink = Arc::new(RecordingSink::default());

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/7")
                .header(REQUEST_ID_HEADER, "request-state-7")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(sink.emit_count(), 0);
    assert_eq!(sink.flush_count(), 0);

    assert!(response.extensions().get::<EmissionOutcome>().is_none());

    let body = body_json(response).await;

    assert_eq!(body["id"], 7);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn sink_failure_keeps_problem_response_intact() {
    let sink = Arc::new(RecordingSink::failing());

    let response = application(Arc::clone(&sink))
        .oneshot(
            Request::builder()
                .uri("/orders/99")
                .header(REQUEST_ID_HEADER, "request-state-99")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    assert_eq!(sink.emit_count(), 1);
    assert_eq!(sink.flush_count(), 0);

    let outcome = response
        .extensions()
        .get::<EmissionOutcome>()
        .expect("emission outcome should be retained");

    match outcome {
        EmissionOutcome::Emitted => {
            panic!("failing sink unexpectedly reported success");
        }

        EmissionOutcome::Failed(error) => {
            assert_eq!(error.kind(), SinkErrorKind::Io);

            assert_eq!(error.message(), "simulated application-state sink failure");
        }
    }

    let problem = body_json(response).await;
    let serialized = serde_json::to_string(&problem).expect("problem response should serialize");

    assert_eq!(problem["status"], 404);
    assert_eq!(problem["request_id"], "request-state-99");

    assert!(!serialized.contains("simulated application-state sink failure"));

    assert!(!serialized.contains("private database lookup failure"));
}

#[test]
fn diagnostic_state_clones_share_the_same_sink_and_reporter_session() {
    let sink = Arc::new(RecordingSink::default());

    let state = DiagnosticState::new(reporter(), sink);

    let cloned = state.clone();

    assert_eq!(
        state.reporter().session_id(),
        cloned.reporter().session_id()
    );

    assert!(Arc::ptr_eq(&state.shared_sink(), &cloned.shared_sink(),));
}
