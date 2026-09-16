use axum::{
    Form, Json, Router,
    body::Body,
    extract::{
        Extension, FromRequest, Path, Query,
        rejection::{
            ExtensionRejection, FormRejection, JsonRejection, PathRejection, QueryRejection,
        },
    },
    http::{Request, StatusCode, header},
    routing::{get, post},
};
use diagprint::{DiagnosticValue, Reporter, Severity};
use diagprint_axum::{AxumRejectionExt, DiagnosticResult};
use http_body_util::BodyExt;
use serde_json::Value;
use std::collections::HashMap;
use tower::ServiceExt;

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-rejection-test")
        .build()
        .expect("test reporter should build")
}

async fn response_json(response: axum::response::Response) -> Value {
    let status = response.status();

    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let mut value: Value = serde_json::from_slice(&bytes).expect("response should contain JSON");

    value["_response_status"] = Value::from(u64::from(status.as_u16()));

    value
}

async fn json_handler(payload: Result<Json<Value>, JsonRejection>) -> DiagnosticResult<StatusCode> {
    match payload {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(rejection) => Err(rejection.to_diagnostic_response(&reporter())),
    }
}

async fn path_handler(value: Result<Path<u64>, PathRejection>) -> DiagnosticResult<StatusCode> {
    match value {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(rejection) => Err(rejection.to_diagnostic_response(&reporter())),
    }
}

async fn query_handler(
    value: Result<Query<HashMap<String, u64>>, QueryRejection>,
) -> DiagnosticResult<StatusCode> {
    match value {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(rejection) => Err(rejection.to_diagnostic_response(&reporter())),
    }
}

async fn form_handler(
    value: Result<Form<HashMap<String, u64>>, FormRejection>,
) -> DiagnosticResult<StatusCode> {
    match value {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(rejection) => Err(rejection.to_diagnostic_response(&reporter())),
    }
}

async fn extension_handler(
    value: Result<Extension<String>, ExtensionRejection>,
) -> DiagnosticResult<StatusCode> {
    match value {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(rejection) => Err(rejection.to_diagnostic_response(&reporter())),
    }
}

#[tokio::test]
async fn json_rejection_preserves_status_but_redacts_detail() {
    let app = Router::new().route("/json", post(json_handler));

    let request = Request::builder()
        .method("POST")
        .uri("/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"name":"#))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("router should respond");

    assert!(response.status().is_client_error());

    let body = response_json(response).await;

    assert_eq!(body["error"]["message"], "Request could not be completed.");

    assert!(body["error"]["report_id"].is_string());
    assert!(body["error"].get("code").is_none());
}

#[tokio::test]
async fn path_rejection_becomes_safe_diagnostic_response() {
    let app = Router::new().route("/items/{id}", get(path_handler));

    let request = Request::builder()
        .uri("/items/not-a-number")
        .body(Body::empty())
        .expect("request should build");

    let response = app.oneshot(request).await.expect("router should respond");

    assert!(response.status().is_client_error());

    let body = response_json(response).await;

    assert_eq!(body["error"]["message"], "Request could not be completed.");
}

#[tokio::test]
async fn query_rejection_becomes_safe_diagnostic_response() {
    let app = Router::new().route("/query", get(query_handler));

    let request = Request::builder()
        .uri("/query?limit=not-a-number")
        .body(Body::empty())
        .expect("request should build");

    let response = app.oneshot(request).await.expect("router should respond");

    assert!(response.status().is_client_error());

    let body = response_json(response).await;

    assert_eq!(body["error"]["message"], "Request could not be completed.");
}

#[tokio::test]
async fn form_rejection_becomes_safe_diagnostic_response() {
    let app = Router::new().route("/form", post(form_handler));

    let request = Request::builder()
        .method("POST")
        .uri("/form")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("count=not-a-number"))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("router should respond");

    assert!(response.status().is_client_error());

    let body = response_json(response).await;

    assert_eq!(body["error"]["message"], "Request could not be completed.");
}

#[tokio::test]
async fn missing_extension_is_treated_as_internal_failure() {
    let app = Router::new().route("/extension", get(extension_handler));

    let request = Request::builder()
        .uri("/extension")
        .body(Body::empty())
        .expect("request should build");

    let response = app.oneshot(request).await.expect("router should respond");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = response_json(response).await;

    assert_eq!(body["error"]["message"], "Internal server error.");
}

#[tokio::test]
async fn rejection_diagnostic_keeps_internal_metadata() {
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"broken":"#))
        .expect("request should build");

    let rejection = Json::<Value>::from_request(request, &())
        .await
        .expect_err("invalid JSON should reject");

    let diagnostic = rejection.to_diagnostic(&reporter());

    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("axum.rejection.json"));

    assert!(diagnostic.attributes.iter().any(|attribute| {
        attribute.name == "axum.rejection.kind"
            && attribute.value == DiagnosticValue::String("json".into())
    }));

    assert!(diagnostic.attributes.iter().any(|attribute| {
        attribute.name == "http.status" && matches!(attribute.value, DiagnosticValue::U64(_))
    }));

    assert!(
        diagnostic
            .message
            .starts_with("Axum json extractor rejected request:")
    );
}
