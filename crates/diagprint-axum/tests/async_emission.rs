#![cfg(feature = "async-delivery")]

use axum::{http::StatusCode, response::IntoResponse};
use diagprint::{Diagnostic, DiagnosticSink, Reporter, Severity, SinkResult};
use diagprint_async::{AsyncDiagnosticSink, AsyncSinkError, BackpressurePolicy};
use diagprint_axum::{AsyncDiagnosticEmissionExt, AsyncEmissionOutcome, DiagnosticResponse};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedDiagnostic {
    report_id: String,
    message: String,
    code: Option<String>,
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
                report_id: diagnostic.report_id.to_string(),
                message: diagnostic.message.clone(),
                code: diagnostic.code.clone(),
            });

        Ok(())
    }
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

            assert!(
                now < deadline,
                "timed out waiting for blocking sink to enter"
            );

            let remaining = deadline.saturating_duration_since(now);

            let (guard, timeout) = self
                .entered_changed
                .wait_timeout(entered, remaining)
                .expect("blocking sink condvar should not be poisoned");

            entered = guard;

            assert!(
                !timeout.timed_out() || *entered >= expected,
                "timed out waiting for blocking sink to enter"
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

fn error_diagnostic() -> Diagnostic {
    let reporter = Reporter::builder()
        .application("diagprint-axum-async-test")
        .build()
        .expect("test reporter should build");

    reporter
        .error("database password secret should never leak")
        .code("internal.database.failure")
}

#[tokio::test]
async fn async_response_enqueues_internal_diagnostic_and_keeps_client_redacted() {
    let recording = RecordingSink::default();

    let sink = AsyncDiagnosticSink::spawn(recording.clone(), 8, BackpressurePolicy::Block)
        .expect("async diagnostic sink should spawn");

    let diagnostic = error_diagnostic();
    let expected_report_id = diagnostic.report_id.to_string();

    let response = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, diagnostic)
        .emit_to_async(&sink)
        .await
        .into_response();

    assert_eq!(
        response.extensions().get::<AsyncEmissionOutcome>(),
        Some(&AsyncEmissionOutcome::Enqueued)
    );

    sink.flush().await.expect("queued diagnostic should flush");

    let observed = recording.observed();

    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].report_id, expected_report_id);
    assert_eq!(
        observed[0].message,
        "database password secret should never leak"
    );
    assert_eq!(
        observed[0].code.as_deref(),
        Some("internal.database.failure")
    );

    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let body: Value = serde_json::from_slice(&bytes).expect("response should contain valid JSON");

    assert_eq!(
        body["error"]["message"],
        Value::String("Internal server error.".into())
    );

    assert!(body["error"].get("code").is_none());

    assert!(
        !String::from_utf8_lossy(&bytes).contains("database password secret should never leak")
    );

    sink.shutdown()
        .await
        .expect("async diagnostic sink should shut down cleanly");
}

#[tokio::test]
async fn queue_rejection_is_metadata_and_does_not_replace_http_response() {
    let state = Arc::new(BlockingState::default());

    let sink = AsyncDiagnosticSink::spawn(
        BlockingSink {
            state: Arc::clone(&state),
        },
        1,
        BackpressurePolicy::Reject,
    )
    .expect("async diagnostic sink should spawn");

    let first = error_diagnostic().emit_to_async(&sink).await;

    assert_eq!(first.outcome(), &AsyncEmissionOutcome::Enqueued);

    state.wait_until_entered(1);

    let second = error_diagnostic().emit_to_async(&sink).await;

    assert_eq!(second.outcome(), &AsyncEmissionOutcome::Enqueued);

    let response = DiagnosticResponse::new(StatusCode::SERVICE_UNAVAILABLE, error_diagnostic())
        .emit_to_async(&sink)
        .await
        .into_response();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let outcome = response
        .extensions()
        .get::<AsyncEmissionOutcome>()
        .expect("async emission outcome should be retained");

    match outcome {
        AsyncEmissionOutcome::Failed(AsyncSinkError::QueueFull { severity }) => {
            assert_eq!(*severity, Severity::Error);
        }

        other => {
            panic!("expected queue-full failure, received {other:?}");
        }
    }

    state.release();

    sink.shutdown()
        .await
        .expect("async diagnostic sink should shut down cleanly");
}

#[tokio::test]
async fn drop_newest_is_reported_explicitly() {
    let state = Arc::new(BlockingState::default());

    let sink = AsyncDiagnosticSink::spawn(
        BlockingSink {
            state: Arc::clone(&state),
        },
        1,
        BackpressurePolicy::DropNewest {
            up_to: Severity::Warning,
        },
    )
    .expect("async diagnostic sink should spawn");

    let first = error_diagnostic().emit_to_async(&sink).await;

    assert_eq!(first.outcome(), &AsyncEmissionOutcome::Enqueued);

    state.wait_until_entered(1);

    let second = error_diagnostic().emit_to_async(&sink).await;

    assert_eq!(second.outcome(), &AsyncEmissionOutcome::Enqueued);

    let reporter = Reporter::builder()
        .application("diagprint-axum-async-test")
        .build()
        .expect("test reporter should build");

    let warning = reporter.warning("optional warning diagnostic");

    let response = DiagnosticResponse::new(StatusCode::BAD_REQUEST, warning)
        .emit_to_async(&sink)
        .await
        .into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    assert_eq!(
        response.extensions().get::<AsyncEmissionOutcome>(),
        Some(&AsyncEmissionOutcome::Dropped)
    );

    state.release();

    sink.shutdown()
        .await
        .expect("async diagnostic sink should shut down cleanly");
}
