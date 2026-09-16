#![cfg(feature = "async-delivery")]

use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware,
    response::Response,
    routing::get,
};
use diagprint::{
    Diagnostic, DiagnosticSink, DiagnosticValue, Reporter, Severity, SinkError, SinkErrorKind,
    SinkResult,
};
use diagprint_async::{AsyncDiagnosticSink, AsyncSinkError, BackpressurePolicy, SubmitOutcome};
use diagprint_axum::{
    ApplicationError, AsyncDiagnosticState, AsyncEmission, AsyncEmissionOutcome,
    ProblemDetailsResponse, REQUEST_ID_HEADER, RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{
    collections::HashSet,
    error::Error,
    fmt,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout};
use tower::ServiceExt;

const CONCURRENT_REQUESTS: usize = 16;

const WORKER_FAILURE_SECRET: &str = "injected-worker-failure-private-secret";

const DIRECT_FLUSH_FAILURE: &str = "injected direct async flush failure";

#[derive(Debug)]
struct OperationalError {
    job_id: u64,
}

impl fmt::Display for OperationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "private asynchronous operational failure for job {}",
            self.job_id
        )
    }
}

impl Error for OperationalError {}

impl ApplicationError for OperationalError {
    fn http_status(&self) -> StatusCode {
        StatusCode::SERVICE_UNAVAILABLE
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .error(format!(
                "private asynchronous operational failure for job {}",
                self.job_id
            ))
            .code("jobs.private_async_failure")
            .attribute("jobs.internal_id", self.job_id.to_string())
    }
}

#[derive(Clone)]
struct AppState {
    diagnostics: AsyncDiagnosticState,
}

async fn failure_handler(
    State(state): State<AppState>,
    context: RequestContext,
    Path(job_id): Path<u64>,
) -> Result<&'static str, AsyncEmission<ProblemDetailsResponse>> {
    Err(state
        .diagnostics
        .emit_problem(&OperationalError { job_id }, &context)
        .await)
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-async-operational")
        .build()
        .expect("operational reporter should build")
}

fn application(sink: Arc<AsyncDiagnosticSink>) -> Router {
    let state = AppState {
        diagnostics: AsyncDiagnosticState::new(reporter(), sink),
    };

    Router::new()
        .route("/fail/{id}", get(failure_handler))
        .route_layer(middleware::from_fn(request_context_middleware))
        .with_state(state)
}

async fn request_failure(
    app: Router,
    job_id: usize,
    request_id: &str,
    query_secret: &str,
) -> Response {
    app.oneshot(
        Request::builder()
            .uri(format!("/fail/{job_id}?token={query_secret}"))
            .header(REQUEST_ID_HEADER, request_id)
            .body(Body::empty())
            .expect("request should build"),
    )
    .await
    .expect("router should respond")
}

fn outcome(response: &Response) -> &AsyncEmissionOutcome {
    response
        .extensions()
        .get::<AsyncEmissionOutcome>()
        .expect("async emission outcome should exist")
}

fn assert_enqueued(response: &Response) {
    assert_eq!(outcome(response), &AsyncEmissionOutcome::Enqueued);
}

fn assert_dropped(response: &Response) {
    assert_eq!(outcome(response), &AsyncEmissionOutcome::Dropped);
}

fn assert_queue_full(response: &Response) {
    match outcome(response) {
        AsyncEmissionOutcome::Failed(AsyncSinkError::QueueFull { severity }) => {
            assert_eq!(*severity, Severity::Error);
        }

        other => {
            panic!("expected queue-full failure, got {other:?}");
        }
    }
}

fn assert_worker_failure(response: &Response, message: &str) {
    match outcome(response) {
        AsyncEmissionOutcome::Failed(AsyncSinkError::Worker { error }) => {
            assert_eq!(error.kind(), SinkErrorKind::Io);

            assert_eq!(error.message(), message);
        }

        other => {
            panic!("expected worker failure, got {other:?}");
        }
    }
}

async fn assert_private_problem(
    response: Response,
    request_id: &str,
    query_secret: &str,
    extra_forbidden: &[&str],
) {
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let problem: Value = serde_json::from_slice(&bytes).expect("response should contain JSON");

    assert_eq!(problem["status"], StatusCode::SERVICE_UNAVAILABLE.as_u16());

    assert_eq!(problem["request_id"], request_id);

    let serialized = String::from_utf8_lossy(&bytes);

    let forbidden = [
        "private asynchronous operational failure",
        "jobs.private_async_failure",
        "jobs.internal_id",
        "diagprint-axum-async-operational",
        "/fail/",
        "token=",
        query_secret,
        "AsyncEmissionOutcome",
        "QueueFull",
    ];

    for value in forbidden.into_iter().chain(extra_forbidden.iter().copied()) {
        assert!(
            !serialized.contains(value),
            "client response leaked forbidden value: {value}"
        );
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
struct GateState {
    entered: Mutex<bool>,
    entered_changed: Condvar,

    released: Mutex<bool>,
    released_changed: Condvar,

    request_ids: Mutex<Vec<Option<String>>>,
}

impl GateState {
    fn wait_until_entered(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);

        let mut entered = self
            .entered
            .lock()
            .expect("gate entered lock should not be poisoned");

        while !*entered {
            let now = Instant::now();

            assert!(now < deadline, "timed out waiting for gate sink");

            let remaining = deadline.saturating_duration_since(now);

            let (guard, wait_result) = self
                .entered_changed
                .wait_timeout(entered, remaining)
                .expect("gate condvar should not be poisoned");

            entered = guard;

            assert!(
                !wait_result.timed_out() || *entered,
                "timed out waiting for gate sink"
            );
        }
    }

    fn release(&self) {
        let mut released = self
            .released
            .lock()
            .expect("gate release lock should not be poisoned");

        *released = true;

        self.released_changed.notify_all();
    }

    fn observed_request_ids(&self) -> Vec<Option<String>> {
        self.request_ids
            .lock()
            .expect("gate observations should not be poisoned")
            .clone()
    }
}

#[derive(Clone)]
struct GateSink {
    state: Arc<GateState>,
}

impl DiagnosticSink for GateSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        self.state
            .request_ids
            .lock()
            .expect("gate observations should not be poisoned")
            .push(string_attribute(diagnostic, "http.request_id"));

        let should_block = {
            let mut entered = self
                .state
                .entered
                .lock()
                .expect("gate entered lock should not be poisoned");

            if *entered {
                false
            } else {
                *entered = true;

                self.state.entered_changed.notify_all();

                true
            }
        };

        if should_block {
            let mut released = self
                .state
                .released
                .lock()
                .expect("gate release lock should not be poisoned");

            while !*released {
                released = self
                    .state
                    .released_changed
                    .wait(released)
                    .expect("gate release condvar should not be poisoned");
            }
        }

        Ok(())
    }
}

struct FailingEmitSink;

impl DiagnosticSink for FailingEmitSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        Err(SinkError::new(SinkErrorKind::Io, WORKER_FAILURE_SECRET))
    }
}

struct FlushFailureSink;

impl DiagnosticSink for FlushFailureSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        Err(SinkError::new(SinkErrorKind::Io, DIRECT_FLUSH_FAILURE))
    }
}

struct PanicSink;

impl DiagnosticSink for PanicSink {
    fn emit(&self, _diagnostic: &Diagnostic) -> SinkResult<()> {
        panic!("injected async sink worker panic");
    }
}

fn prefill_diagnostic(label: &str) -> Diagnostic {
    reporter()
        .info(format!("operational queue prefill {label}"))
        .code("operational.queue_prefill")
}

async fn saturate_queue(sink: &AsyncDiagnosticSink, gate: &GateState) {
    assert_eq!(
        sink.emit(prefill_diagnostic("worker",),)
            .await
            .expect("worker prefill should enqueue",),
        SubmitOutcome::Enqueued
    );

    gate.wait_until_entered();

    assert_eq!(
        sink.emit(prefill_diagnostic("queued",),)
            .await
            .expect("queue prefill should enqueue",),
        SubmitOutcome::Enqueued
    );

    assert_eq!(
        sink.capacity(),
        0,
        "queue should be deterministically saturated"
    );
}

fn take_sink(sink: Arc<AsyncDiagnosticSink>) -> AsyncDiagnosticSink {
    match Arc::try_unwrap(sink) {
        Ok(sink) => sink,

        Err(sink) => {
            panic!(
                "async sink still has {} strong references",
                Arc::strong_count(&sink)
            );
        }
    }
}

async fn shutdown_ok(sink: Arc<AsyncDiagnosticSink>) {
    take_sink(sink)
        .shutdown()
        .await
        .expect("async diagnostic sink should shut down cleanly");
}

fn assert_worker_error(error: &AsyncSinkError, expected_message: &str) {
    match error {
        AsyncSinkError::Worker { error } => {
            assert_eq!(error.kind(), SinkErrorKind::Io);

            assert_eq!(error.message(), expected_message);
        }

        other => {
            panic!("expected worker error, got {other:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn primitive_emit_failure_is_sticky_and_shutdown_reports_it() {
    let sink = AsyncDiagnosticSink::spawn(FailingEmitSink, 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    assert_eq!(
        sink.emit(reporter().error("primitive emit-failure diagnostic",),)
            .await
            .expect("queue acceptance precedes worker delivery",),
        SubmitOutcome::Enqueued
    );

    let flush_error = sink
        .flush()
        .await
        .expect_err("worker emit failure must fail flush");

    assert_worker_error(&flush_error, WORKER_FAILURE_SECRET);

    let later_error = sink
        .emit(reporter().error("primitive diagnostic after worker failure"))
        .await
        .expect_err("worker failure must remain sticky");

    assert_worker_error(&later_error, WORKER_FAILURE_SECRET);

    let shutdown_error = sink
        .shutdown()
        .await
        .expect_err("shutdown must retain worker failure");

    assert_worker_error(&shutdown_error, WORKER_FAILURE_SECRET);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn primitive_flush_failure_is_sticky_and_blocks_future_submission() {
    let sink = AsyncDiagnosticSink::spawn(FlushFailureSink, 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    assert_eq!(
        sink.emit(reporter().warning("primitive diagnostic before flush failure",),)
            .await
            .expect("diagnostic should enter queue",),
        SubmitOutcome::Enqueued
    );

    let flush_error = sink
        .flush()
        .await
        .expect_err("injected flush failure must surface");

    assert_worker_error(&flush_error, DIRECT_FLUSH_FAILURE);

    let later_error = sink
        .emit(reporter().warning("primitive diagnostic after flush failure"))
        .await
        .expect_err("failed worker must reject later submissions");

    assert_worker_error(&later_error, DIRECT_FLUSH_FAILURE);

    let shutdown_error = sink
        .shutdown()
        .await
        .expect_err("shutdown must preserve flush failure");

    assert_worker_error(&shutdown_error, DIRECT_FLUSH_FAILURE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn primitive_worker_panic_closes_submission_and_surfaces_join_failure() {
    let sink = AsyncDiagnosticSink::spawn(PanicSink, 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    assert_eq!(
        sink.emit(reporter().error("primitive diagnostic that triggers worker panic",),)
            .await
            .expect("queue acceptance does not imply delivery success",),
        SubmitOutcome::Enqueued
    );

    let closed = sink
        .flush()
        .await
        .expect_err("panicked worker must close the queue");

    assert_eq!(closed, AsyncSinkError::Closed);

    let later = sink
        .emit(reporter().error("primitive diagnostic after worker panic"))
        .await
        .expect_err("closed worker must reject later submission");

    assert_eq!(later, AsyncSinkError::Closed);

    let shutdown = sink
        .shutdown()
        .await
        .expect_err("worker panic must surface at lifecycle join");

    assert!(
        matches!(shutdown, AsyncSinkError::Join { .. }),
        "expected Join error after worker panic, got {shutdown:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worker_failure_remains_server_metadata_and_never_replaces_http_response() {
    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(FailingEmitSink, 8, BackpressurePolicy::Block)
            .expect("async diagnostic sink should spawn"),
    );

    let first_request_id = "worker-failure-first";

    let first_secret = "query-worker-failure-first";

    let first = request_failure(
        application(Arc::clone(&sink)),
        1,
        first_request_id,
        first_secret,
    )
    .await;

    assert_enqueued(&first);

    assert_private_problem(
        first,
        first_request_id,
        first_secret,
        &[WORKER_FAILURE_SECRET],
    )
    .await;

    let failure = sink
        .flush()
        .await
        .expect_err("worker failure must surface at flush");

    assert_worker_error(&failure, WORKER_FAILURE_SECRET);

    let second_request_id = "worker-failure-second";

    let second_secret = "query-worker-failure-second";

    let second = request_failure(
        application(Arc::clone(&sink)),
        2,
        second_request_id,
        second_secret,
    )
    .await;

    assert_worker_failure(&second, WORKER_FAILURE_SECRET);

    assert_private_problem(
        second,
        second_request_id,
        second_secret,
        &[WORKER_FAILURE_SECRET],
    )
    .await;

    let shutdown_error = take_sink(sink)
        .shutdown()
        .await
        .expect_err("failed worker should fail shutdown");

    assert_worker_error(&shutdown_error, WORKER_FAILURE_SECRET);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reject_pressure_preserves_every_http_response() {
    let gate = Arc::new(GateState::default());

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(
            GateSink {
                state: Arc::clone(&gate),
            },
            1,
            BackpressurePolicy::Reject,
        )
        .expect("async diagnostic sink should spawn"),
    );

    saturate_queue(sink.as_ref(), gate.as_ref()).await;

    let app = application(Arc::clone(&sink));

    let handles = (0..CONCURRENT_REQUESTS)
        .map(|index| {
            let app = app.clone();

            tokio::spawn(async move {
                let request_id = format!("reject-{index}");

                let secret = format!("reject-secret-{index}");

                let response = request_failure(app, index, &request_id, &secret).await;

                (request_id, secret, response)
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        let (request_id, secret, response) = timeout(Duration::from_secs(5), handle)
            .await
            .expect("reject request should not block")
            .expect("reject request task should not panic");

        assert_queue_full(&response);

        assert_private_problem(response, &request_id, &secret, &[]).await;
    }

    assert_eq!(
        gate.observed_request_ids().len(),
        1,
        "only the blocked worker diagnostic should have reached the sink"
    );

    gate.release();

    sink.flush()
        .await
        .expect("prefilled diagnostics should flush");

    assert_eq!(
        gate.observed_request_ids().len(),
        2,
        "rejected request diagnostics must never reach the underlying sink"
    );

    drop(app);

    shutdown_ok(sink).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_drop_newest_pressure_is_explicit_and_never_delivers_dropped_requests() {
    let gate = Arc::new(GateState::default());

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(
            GateSink {
                state: Arc::clone(&gate),
            },
            1,
            BackpressurePolicy::DropNewest {
                up_to: Severity::Error,
            },
        )
        .expect("async diagnostic sink should spawn"),
    );

    saturate_queue(sink.as_ref(), gate.as_ref()).await;

    let app = application(Arc::clone(&sink));

    let handles = (0..CONCURRENT_REQUESTS)
        .map(|index| {
            let app = app.clone();

            tokio::spawn(async move {
                let request_id = format!("drop-{index}");

                let secret = format!("drop-secret-{index}");

                let response = request_failure(app, index, &request_id, &secret).await;

                (request_id, secret, response)
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        let (request_id, secret, response) = timeout(Duration::from_secs(5), handle)
            .await
            .expect("drop-newest request should not block")
            .expect("drop-newest task should not panic");

        assert_dropped(&response);

        assert_private_problem(response, &request_id, &secret, &[]).await;
    }

    assert_eq!(gate.observed_request_ids().len(), 1);

    gate.release();

    sink.flush()
        .await
        .expect("prefilled diagnostics should flush");

    assert_eq!(
        gate.observed_request_ids().len(),
        2,
        "dropped request diagnostics must never reach the underlying sink"
    );

    drop(app);

    shutdown_ok(sink).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn block_backpressure_waits_for_capacity_then_delivers_every_concurrent_diagnostic() {
    let gate = Arc::new(GateState::default());

    let sink = Arc::new(
        AsyncDiagnosticSink::spawn(
            GateSink {
                state: Arc::clone(&gate),
            },
            1,
            BackpressurePolicy::Block,
        )
        .expect("async diagnostic sink should spawn"),
    );

    saturate_queue(sink.as_ref(), gate.as_ref()).await;

    let app = application(Arc::clone(&sink));

    let handles = (0..CONCURRENT_REQUESTS)
        .map(|index| {
            let app = app.clone();

            tokio::spawn(async move {
                let request_id = format!("block-{index}");

                let secret = format!("block-secret-{index}");

                let response = request_failure(app, index, &request_id, &secret).await;

                (request_id, secret, response)
            })
        })
        .collect::<Vec<_>>();

    sleep(Duration::from_millis(100)).await;

    assert!(
        handles.iter().all(|handle| { !handle.is_finished() },),
        "Block policy returned an HTTP response before queue capacity became available"
    );

    assert_eq!(gate.observed_request_ids().len(), 1);

    gate.release();

    let mut expected_request_ids = HashSet::new();

    for handle in handles {
        let (request_id, secret, response) = timeout(Duration::from_secs(5), handle)
            .await
            .expect("blocked request should complete after release")
            .expect("blocked request task should not panic");

        assert_enqueued(&response);

        expected_request_ids.insert(request_id.clone());

        assert_private_problem(response, &request_id, &secret, &[]).await;
    }

    sink.flush()
        .await
        .expect("all accepted diagnostics should flush");

    let observed = gate.observed_request_ids();

    assert_eq!(
        observed.len(),
        CONCURRENT_REQUESTS + 2,
        "every accepted diagnostic must be delivered exactly once"
    );

    let observed_request_ids = observed.into_iter().flatten().collect::<HashSet<_>>();

    assert_eq!(
        observed_request_ids, expected_request_ids,
        "request correlation must survive concurrent Block backpressure"
    );

    drop(app);

    shutdown_ok(sink).await;
}
