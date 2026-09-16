use axum::{
    Json, Router,
    body::Body,
    http::{Method, Request, StatusCode},
    middleware,
    routing::get,
};
use diagprint::{Diagnostic, DiagnosticValue, Reporter};
use diagprint_axum::{
    ApplicationError, ApplicationErrorExt, InvalidRequestId, REQUEST_ID_HEADER, RequestContext,
    RequestId, request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{error::Error, fmt};
use tower::ServiceExt;

async fn context_handler(context: RequestContext) -> Json<Value> {
    Json(json!({
        "request_id": context.request_id().as_str(),
        "method": context.method().as_str(),
        "matched_route": context.matched_route(),
    }))
}

fn context_router() -> Router {
    Router::new()
        .route("/users/{id}", get(context_handler))
        .route_layer(middleware::from_fn(request_context_middleware))
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-context-test")
        .build()
        .expect("test reporter should build")
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

fn attribute<'a>(diagnostic: &'a Diagnostic, name: &str) -> Option<&'a DiagnosticValue> {
    diagnostic
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .map(|attribute| &attribute.value)
}

#[tokio::test]
async fn valid_inbound_request_id_is_preserved_and_propagated() {
    let response = context_router()
        .oneshot(
            Request::builder()
                .uri("/users/42")
                .header(REQUEST_ID_HEADER, "client-request-123")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("client-request-123")
    );

    let json = body_json(response).await;

    assert_eq!(json["request_id"], "client-request-123");
    assert_eq!(json["method"], "GET");
    assert_eq!(json["matched_route"], "/users/{id}");
}

#[tokio::test]
async fn missing_request_context_rejects_without_exposing_configuration_details() {
    let app = Router::new().route("/users/{id}", get(context_handler));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/users/42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(response.headers().get(REQUEST_ID_HEADER).is_none());

    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let body = String::from_utf8(body.to_vec()).expect("rejection body should be valid UTF-8");

    assert!(!body.contains("request_context_middleware"));
    assert!(!body.contains("RequestContext"));
    assert!(!body.contains("middleware"));
}

#[tokio::test]
async fn invalid_inbound_request_id_is_replaced() {
    let response = context_router()
        .oneshot(
            Request::builder()
                .uri("/users/42")
                .header(REQUEST_ID_HEADER, "bad/request/id")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    let selected = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .expect("response should contain request ID")
        .to_owned();

    assert_ne!(selected, "bad/request/id");
    assert!(RequestId::new(selected.as_str()).is_ok());

    let json = body_json(response).await;

    assert_eq!(json["request_id"], selected);
}

#[tokio::test]
async fn request_context_does_not_capture_raw_path_or_query_string() {
    let response = context_router()
        .oneshot(
            Request::builder()
                .uri("/users/42?token=super-secret")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    let json = body_json(response).await;
    let serialized = serde_json::to_string(&json).expect("JSON should serialize");

    assert_eq!(json["matched_route"], "/users/{id}");

    assert!(!serialized.contains("/users/42"));
    assert!(!serialized.contains("super-secret"));
    assert!(!serialized.contains("token="));
}

#[test]
fn request_id_validation_is_strict_and_deterministic() {
    assert!(RequestId::new("request-123").is_ok());
    assert!(RequestId::new("abc_DEF.123:xyz").is_ok());

    for value in [
        "",
        "bad/request",
        "bad request",
        "bad?request",
        "bad#request",
    ] {
        assert_eq!(RequestId::new(value), Err(InvalidRequestId));
    }

    let oversized = "a".repeat(129);

    assert_eq!(RequestId::new(oversized), Err(InvalidRequestId));
}

#[test]
fn request_context_adds_only_safe_structured_attributes() {
    let context = RequestContext::new(
        RequestId::new("request-abc-123").expect("request ID should be valid"),
        Method::POST,
        Some("/orders/{id}".to_owned()),
    );

    let diagnostic = context.annotate_diagnostic(reporter().error("order failed"));

    assert!(matches!(
        attribute(&diagnostic, "http.request_id"),
        Some(DiagnosticValue::String(value))
            if value == "request-abc-123"
    ));

    assert!(matches!(
        attribute(&diagnostic, "http.method"),
        Some(DiagnosticValue::String(value))
            if value == "POST"
    ));

    assert!(matches!(
        attribute(&diagnostic, "http.route"),
        Some(DiagnosticValue::String(value))
            if value == "/orders/{id}"
    ));

    assert_eq!(diagnostic.attributes.len(), 3);
}

#[derive(Debug)]
struct ContextApplicationError;

impl fmt::Display for ContextApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("private domain failure")
    }
}

impl Error for ContextApplicationError {}

impl ApplicationError for ContextApplicationError {
    fn http_status(&self) -> StatusCode {
        StatusCode::CONFLICT
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .warning("private domain failure")
            .code("application.conflict")
    }
}

#[test]
fn application_error_context_correlates_problem_and_diagnostic() {
    let context = RequestContext::new(
        RequestId::new("request-conflict-123").expect("request ID should be valid"),
        Method::PUT,
        Some("/orders/{id}".to_owned()),
    );

    let response = ContextApplicationError.to_problem_response_with_context(&reporter(), &context);

    let problem = response.problem_details();
    let diagnostic = response.diagnostic_response().diagnostic();

    assert_eq!(problem.request_id.as_deref(), Some("request-conflict-123"));

    assert_ne!(
        problem.request_id, problem.report_id,
        "request and diagnostic report identity must remain distinct"
    );

    assert!(matches!(
        attribute(diagnostic, "http.request_id"),
        Some(DiagnosticValue::String(value))
            if value == "request-conflict-123"
    ));

    assert!(matches!(
        attribute(diagnostic, "http.method"),
        Some(DiagnosticValue::String(value))
            if value == "PUT"
    ));

    assert!(matches!(
        attribute(diagnostic, "http.route"),
        Some(DiagnosticValue::String(value))
            if value == "/orders/{id}"
    ));

    assert_eq!(problem.detail, "Request could not be completed.");
    assert_eq!(problem.code, None);
}
