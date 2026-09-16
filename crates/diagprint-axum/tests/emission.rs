use axum::{
    http::{Method, StatusCode},
    response::IntoResponse,
};
use diagprint::{
    Diagnostic, DiagnosticSink, DiagnosticValue, Reporter, SinkError, SinkErrorKind, SinkResult,
};
use diagprint_axum::{
    DiagnosticEmissionExt, DiagnosticResponse, DiagnosticResponseExt, EmissionOutcome,
    ProblemDetailsResponseExt, RequestContext, RequestId,
};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
struct RecordingSink {
    diagnostics: Arc<Mutex<Vec<Diagnostic>>>,
    flushes: Arc<Mutex<usize>>,
}

impl RecordingSink {
    fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics
            .lock()
            .expect("recording sink lock should not be poisoned")
            .clone()
    }

    fn flush_count(&self) -> usize {
        *self
            .flushes
            .lock()
            .expect("flush counter lock should not be poisoned")
    }
}

impl DiagnosticSink for RecordingSink {
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        self.diagnostics
            .lock()
            .expect("recording sink lock should not be poisoned")
            .push(diagnostic.clone());

        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        let mut flushes = self
            .flushes
            .lock()
            .expect("flush counter lock should not be poisoned");

        *flushes += 1;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct FailingSink;

impl DiagnosticSink for FailingSink {
    fn emit(&self, _: &Diagnostic) -> SinkResult<()> {
        Err(SinkError::new(
            SinkErrorKind::Io,
            "test diagnostic sink unavailable",
        ))
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-emission-test")
        .build()
        .expect("test reporter should build")
}

fn attribute<'a>(diagnostic: &'a Diagnostic, name: &str) -> Option<&'a DiagnosticValue> {
    diagnostic
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .map(|attribute| &attribute.value)
}

#[test]
fn constructing_response_does_not_emit_implicitly() {
    let sink = RecordingSink::default();

    let _response = reporter()
        .error("internal failure")
        .into_http_response(StatusCode::INTERNAL_SERVER_ERROR);

    assert!(sink.diagnostics().is_empty());
    assert_eq!(sink.flush_count(), 0);
}

#[test]
fn diagnostic_response_emits_exact_internal_diagnostic() {
    let sink = RecordingSink::default();

    let response = reporter()
        .error("database connection failed")
        .code("database.unavailable")
        .into_http_response(StatusCode::SERVICE_UNAVAILABLE);

    let expected_report_id = response.diagnostic().report_id;

    let emission = response.emit_to(&sink);

    assert!(emission.outcome().is_emitted());

    let diagnostics = sink.diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].report_id, expected_report_id);
    assert_eq!(diagnostics[0].message, "database connection failed");
    assert_eq!(diagnostics[0].code.as_deref(), Some("database.unavailable"));
}

#[test]
fn explicit_emission_does_not_flush_sink() {
    let sink = RecordingSink::default();

    let emission = reporter().warning("cache is stale").emit_to(&sink);

    assert!(emission.outcome().is_emitted());
    assert_eq!(sink.diagnostics().len(), 1);
    assert_eq!(sink.flush_count(), 0);
}

#[test]
fn correlated_problem_response_emits_request_context() {
    let sink = RecordingSink::default();

    let context = RequestContext::new(
        RequestId::new("request-emission-123").expect("request ID should be valid"),
        Method::PUT,
        Some("/orders/{id}".to_owned()),
    );

    let diagnostic =
        context.annotate_diagnostic(reporter().warning("order conflict").code("orders.conflict"));

    let problem = DiagnosticResponse::new(StatusCode::CONFLICT, diagnostic)
        .into_problem_details()
        .with_request_id(context.request_id().as_str());

    let expected_report_id = problem.diagnostic_response().diagnostic().report_id;

    let emission = problem.emit_to(&sink);

    assert!(emission.outcome().is_emitted());

    let diagnostics = sink.diagnostics();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].report_id, expected_report_id);

    assert!(matches!(
        attribute(&diagnostics[0], "http.request_id"),
        Some(DiagnosticValue::String(value))
            if value == "request-emission-123"
    ));

    assert!(matches!(
        attribute(&diagnostics[0], "http.method"),
        Some(DiagnosticValue::String(value))
            if value == "PUT"
    ));

    assert!(matches!(
        attribute(&diagnostics[0], "http.route"),
        Some(DiagnosticValue::String(value))
            if value == "/orders/{id}"
    ));

    let problem = emission.value().problem_details();

    assert_eq!(problem.request_id.as_deref(), Some("request-emission-123"));

    let expected_report_id = expected_report_id.to_string();

    assert_eq!(
        problem.report_id.as_deref(),
        Some(expected_report_id.as_str())
    );
}

#[test]
fn sink_failure_is_data_and_preserves_response() {
    let response = reporter()
        .error("database password was rejected")
        .code("database.secret_failure")
        .into_http_response(StatusCode::INTERNAL_SERVER_ERROR);

    let emission = response.emit_to(&FailingSink);

    assert!(emission.outcome().is_failed());

    let error = emission
        .outcome()
        .error()
        .expect("failed emission should contain sink error");

    assert_eq!(error.kind(), SinkErrorKind::Io);
    assert_eq!(error.message(), "test diagnostic sink unavailable");

    let client = emission.value().client_body();

    assert_eq!(client.error.status, 500);
    assert_eq!(client.error.message, "Internal server error.");
    assert_eq!(client.error.code, None);
}

#[tokio::test]
async fn failed_emission_remains_returnable_as_axum_response() {
    let emission = reporter()
        .error("private implementation failure")
        .code("private.failure")
        .into_http_response(StatusCode::INTERNAL_SERVER_ERROR)
        .emit_to(&FailingSink);

    let response = emission.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let outcome = response
        .extensions()
        .get::<EmissionOutcome>()
        .expect("response should retain emission outcome");

    assert!(outcome.is_failed());

    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let body = String::from_utf8(body.to_vec()).expect("response body should be UTF-8 JSON");

    assert!(body.contains("Internal server error."));
    assert!(!body.contains("private implementation failure"));
    assert!(!body.contains("private.failure"));
}
