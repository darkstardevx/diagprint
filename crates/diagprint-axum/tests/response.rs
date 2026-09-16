use axum::{
    http::{StatusCode, header},
    response::IntoResponse,
};
use diagprint::{Diagnostic, Reporter};
use diagprint_axum::{DiagnosticResponse, DiagnosticResponseExt, ResponsePolicy};

fn test_diagnostic() -> Diagnostic {
    let reporter = Reporter::builder()
        .application("diagprint-axum-test")
        .build()
        .expect("test reporter should build");

    reporter
        .error("database password secret should never leak")
        .code("internal.database.failure")
}

#[test]
fn default_policy_redacts_internal_message_and_code() {
    let diagnostic = test_diagnostic();
    let report_id = diagnostic.report_id.to_string();

    let response = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, diagnostic);

    let body = response.client_body();

    assert_eq!(body.error.status, 500);
    assert_eq!(body.error.message, "Internal server error.");
    assert_eq!(body.error.report_id.as_deref(), Some(report_id.as_str()));
    assert_eq!(body.error.code, None);
}

#[test]
fn client_errors_receive_client_safe_fallback_message() {
    let response = DiagnosticResponse::new(StatusCode::BAD_REQUEST, test_diagnostic());

    let body = response.client_body();

    assert_eq!(body.error.status, 400);
    assert_eq!(body.error.message, "Request could not be completed.");
}

#[test]
fn diagnostic_message_and_code_require_explicit_opt_in() {
    let policy = ResponsePolicy::default()
        .expose_diagnostic_message(true)
        .expose_diagnostic_code(true);

    let response = DiagnosticResponse::new(StatusCode::SERVICE_UNAVAILABLE, test_diagnostic())
        .with_policy(policy);

    let body = response.client_body();

    assert_eq!(
        body.error.message,
        "database password secret should never leak"
    );
    assert_eq!(
        body.error.code.as_deref(),
        Some("internal.database.failure")
    );
}

#[test]
fn report_id_can_be_omitted() {
    let policy = ResponsePolicy::default().include_report_id(false);

    let response = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, test_diagnostic())
        .with_policy(policy);

    assert_eq!(response.client_body().error.report_id, None);
}

#[test]
fn fallback_messages_are_configurable() {
    let policy = ResponsePolicy::default()
        .client_error_message("Invalid request.")
        .server_error_message("Service temporarily unavailable.");

    let client = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .with_policy(policy.clone());

    let server = DiagnosticResponse::new(StatusCode::SERVICE_UNAVAILABLE, test_diagnostic())
        .with_policy(policy);

    assert_eq!(client.client_body().error.message, "Invalid request.");

    assert_eq!(
        server.client_body().error.message,
        "Service temporarily unavailable."
    );
}

#[test]
fn extension_trait_builds_response() {
    let response = test_diagnostic().into_http_response(StatusCode::CONFLICT);

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn into_response_preserves_status_and_disables_caching() {
    let response = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, test_diagnostic())
        .into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
}

#[test]
fn diagnostic_response_remains_compact_for_handler_results() {
    assert!(
        std::mem::size_of::<DiagnosticResponse>() <= 128,
        "DiagnosticResponse grew too large for ergonomic Result error usage"
    );
}
