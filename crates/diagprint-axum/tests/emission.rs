use axum::{
    http::{StatusCode, header},
    response::IntoResponse,
};
use diagprint::{Diagnostic, DiagnosticSink, Reporter, SinkError, SinkErrorKind, SinkResult};
use diagprint_axum::{
    DiagnosticEmissionExt, DiagnosticResponse, EmissionOutcome, ProblemDetailsResponseExt,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedDiagnostic {
    report_id: String,
    message: String,
    code: Option<String>,
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
                "simulated diagnostic sink failure",
            ));
        }

        self.observed
            .lock()
            .expect("test sink lock should not be poisoned")
            .push(ObservedDiagnostic {
                report_id: diagnostic.report_id.to_string(),
                message: diagnostic.message.clone(),
                code: diagnostic.code.clone(),
            });

        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        self.flush_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn test_diagnostic() -> Diagnostic {
    let reporter = Reporter::builder()
        .application("diagprint-axum-emission-test")
        .build()
        .expect("test reporter should build");

    reporter
        .error("database password secret should never leak")
        .code("internal.database.failure")
}

#[test]
fn constructing_response_does_not_emit_automatically() {
    let sink = RecordingSink::default();

    let _response = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, test_diagnostic());

    assert_eq!(sink.emit_count(), 0);
    assert_eq!(sink.flush_count(), 0);
    assert!(sink.observed().is_empty());
}

#[test]
fn diagnostic_can_be_emitted_explicitly() {
    let sink = RecordingSink::default();
    let diagnostic = test_diagnostic();

    let expected_report_id = diagnostic.report_id.to_string();

    let emission = diagnostic.emit_to(&sink);

    assert_eq!(emission.outcome(), &EmissionOutcome::Emitted);
    assert_eq!(sink.emit_count(), 1);
    assert_eq!(sink.flush_count(), 0);

    let observed = sink.observed();

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
}

#[tokio::test]
async fn diagnostic_response_emits_internal_diagnostic_but_keeps_client_redacted() {
    let sink = RecordingSink::default();

    let response = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, test_diagnostic())
        .emit_to(&sink)
        .into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

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
}

#[tokio::test]
async fn problem_details_response_supports_explicit_emission() {
    let sink = RecordingSink::default();

    let response = DiagnosticResponse::new(StatusCode::BAD_GATEWAY, test_diagnostic())
        .into_problem_details()
        .emit_to(&sink)
        .into_response();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static(
            "application/problem+json"
        ))
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
        "database password secret should never leak"
    );

    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let body: Value = serde_json::from_slice(&bytes).expect("response should contain valid JSON");

    assert_eq!(
        body["detail"],
        Value::String("Internal server error.".into())
    );

    assert!(
        !String::from_utf8_lossy(&bytes).contains("database password secret should never leak")
    );
}

#[test]
fn sink_failure_becomes_response_metadata_without_preventing_response() {
    let sink = RecordingSink::failing();

    let response = DiagnosticResponse::new(StatusCode::SERVICE_UNAVAILABLE, test_diagnostic())
        .emit_to(&sink)
        .into_response();

    assert_eq!(sink.emit_count(), 1);
    assert_eq!(sink.flush_count(), 0);

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let outcome = response
        .extensions()
        .get::<EmissionOutcome>()
        .expect("emission outcome should be retained in response extensions");

    match outcome {
        EmissionOutcome::Emitted => {
            panic!("failing sink unexpectedly reported successful emission");
        }
        EmissionOutcome::Failed(error) => {
            assert_eq!(error.kind(), SinkErrorKind::Io);
            assert_eq!(error.message(), "simulated diagnostic sink failure");
        }
    }
}

#[test]
fn emit_to_performs_one_attempt_and_never_flushes() {
    let sink = RecordingSink::failing();

    let emission = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, test_diagnostic())
        .emit_to(&sink);

    assert!(emission.outcome().is_failed());
    assert_eq!(sink.emit_count(), 1);
    assert_eq!(sink.flush_count(), 0);
}
