use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use diagprint::{Diagnostic, Reporter};
use diagprint_axum::{
    ABOUT_BLANK, DiagnosticResponse, PROBLEM_JSON_MEDIA_TYPE, ProblemDetailsPolicy,
    ProblemDetailsResponse, ProblemDetailsResponseExt, ResponsePolicy,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn test_diagnostic() -> Diagnostic {
    let reporter = Reporter::builder()
        .application("diagprint-axum-problem-test")
        .build()
        .expect("test reporter should build");

    reporter
        .error("database password secret should never leak")
        .code("internal.database.failure")
}

#[test]
fn default_problem_details_are_privacy_safe() {
    let diagnostic = test_diagnostic();
    let report_id = diagnostic.report_id.to_string();

    let problem = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, diagnostic)
        .into_problem_details()
        .problem_details();

    assert_eq!(problem.type_uri, ABOUT_BLANK);
    assert_eq!(problem.title, "Internal Server Error");
    assert_eq!(problem.status, 500);
    assert_eq!(problem.detail, "Internal server error.");
    assert_eq!(problem.report_id.as_deref(), Some(report_id.as_str()));
    assert_eq!(problem.request_id, None);
    assert_eq!(problem.code, None);

    assert!(!problem.detail.contains("password"));
    assert!(!problem.detail.contains("database"));
}

#[test]
fn client_problem_uses_http_status_title_and_safe_detail() {
    let problem = DiagnosticResponse::new(StatusCode::BAD_REQUEST, test_diagnostic())
        .into_problem_details()
        .problem_details();

    assert_eq!(problem.type_uri, ABOUT_BLANK);
    assert_eq!(problem.title, "Bad Request");
    assert_eq!(problem.status, 400);
    assert_eq!(problem.detail, "Request could not be completed.");
}

#[test]
fn diagnostic_message_and_code_still_require_explicit_opt_in() {
    let response_policy = ResponsePolicy::default()
        .expose_diagnostic_message(true)
        .expose_diagnostic_code(true);

    let problem = DiagnosticResponse::new(StatusCode::SERVICE_UNAVAILABLE, test_diagnostic())
        .with_policy(response_policy)
        .into_problem_details()
        .problem_details();

    assert_eq!(problem.detail, "database password secret should never leak");

    assert_eq!(problem.code.as_deref(), Some("internal.database.failure"));
}

#[test]
fn report_id_visibility_still_comes_from_response_policy() {
    let response_policy = ResponsePolicy::default().include_report_id(false);

    let problem = DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, test_diagnostic())
        .with_policy(response_policy)
        .into_problem_details()
        .problem_details();

    assert_eq!(problem.report_id, None);
}

#[test]
fn request_and_report_ids_remain_distinct() {
    let diagnostic = test_diagnostic();
    let report_id = diagnostic.report_id.to_string();

    let problem = DiagnosticResponse::new(StatusCode::BAD_REQUEST, diagnostic)
        .into_problem_details()
        .with_request_id("request-123")
        .problem_details();

    assert_eq!(problem.report_id.as_deref(), Some(report_id.as_str()));
    assert_eq!(problem.request_id.as_deref(), Some("request-123"));
    assert_ne!(problem.report_id, problem.request_id);
}

#[test]
fn request_id_can_be_suppressed_by_problem_policy() {
    let policy = ProblemDetailsPolicy::default().include_request_id(false);

    let problem = DiagnosticResponse::new(StatusCode::BAD_REQUEST, test_diagnostic())
        .into_problem_details()
        .with_request_id("request-123")
        .with_problem_policy(policy)
        .problem_details();

    assert_eq!(problem.request_id, None);
}

#[test]
fn custom_problem_type_title_and_instance_are_supported() {
    let policy = ProblemDetailsPolicy::default()
        .with_type_uri("https://api.example.com/problems/invalid-order")
        .with_title("Invalid order")
        .with_instance("/orders/42/problems/7");

    let problem = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .into_problem_details()
        .with_problem_policy(policy)
        .problem_details();

    assert_eq!(
        problem.type_uri,
        "https://api.example.com/problems/invalid-order"
    );
    assert_eq!(problem.title, "Invalid order");
    assert_eq!(problem.status, 422);
    assert_eq!(problem.instance.as_deref(), Some("/orders/42/problems/7"));
}

#[test]
fn into_response_uses_problem_json_and_disables_caching() {
    let response = DiagnosticResponse::new(StatusCode::BAD_REQUEST, test_diagnostic())
        .into_problem_details()
        .into_response();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static(PROBLEM_JSON_MEDIA_TYPE))
    );

    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&header::HeaderValue::from_static("no-store"))
    );
}

async fn problem_handler() -> ProblemDetailsResponse {
    DiagnosticResponse::new(StatusCode::CONFLICT, test_diagnostic())
        .into_problem_details()
        .with_request_id("request-router-test")
}

#[tokio::test]
async fn axum_handler_returns_rfc9457_document() {
    let app = Router::new().route("/problem", get(problem_handler));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/problem")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::CONFLICT);

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static(PROBLEM_JSON_MEDIA_TYPE))
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("problem response body should collect")
        .to_bytes();

    let json: Value = serde_json::from_slice(&body).expect("problem response should be valid JSON");

    assert_eq!(json["type"], ABOUT_BLANK);
    assert_eq!(json["title"], "Conflict");
    assert_eq!(json["status"], 409);
    assert_eq!(json["detail"], "Request could not be completed.");
    assert_eq!(json["request_id"], "request-router-test");

    assert!(json.get("report_id").is_some());
    assert!(json.get("code").is_none());

    let serialized = String::from_utf8(body.to_vec()).expect("body should be UTF-8");

    assert!(!serialized.contains("database password"));
    assert!(!serialized.contains("internal.database.failure"));
}

#[test]
fn public_extensions_are_serialized_at_the_problem_root() {
    let response = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .into_problem_details()
        .with_extension(
            "errors",
            serde_json::json!([
                {
                    "field": "email",
                    "code": "invalid_format"
                }
            ]),
        )
        .expect("public extension should be accepted");

    let problem = response.problem_details();

    assert_eq!(
        problem.extensions().get("errors"),
        Some(&serde_json::json!([
            {
                "field": "email",
                "code": "invalid_format"
            }
        ]))
    );

    let json = serde_json::to_value(problem).expect("Problem Details should serialize");

    assert!(json.get("extensions").is_none());
    assert_eq!(json["errors"][0]["field"], "email");
    assert_eq!(json["errors"][0]["code"], "invalid_format");
}

#[test]
fn internal_extensions_are_redacted_by_default() {
    let response = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .into_problem_details()
        .with_internal_extension(
            "debug_state",
            serde_json::json!({
                "validator": "email",
                "stage": 3
            }),
        )
        .expect("internal extension should be accepted");

    let json =
        serde_json::to_value(response.problem_details()).expect("Problem Details should serialize");

    assert!(json.get("debug_state").is_none());
}

#[test]
fn internal_extensions_require_explicit_policy_opt_in() {
    let policy = ProblemDetailsPolicy::default().expose_internal_extensions(true);

    let response = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .into_problem_details()
        .with_internal_extension(
            "debug_state",
            serde_json::json!({
                "validator": "email",
                "stage": 3
            }),
        )
        .expect("internal extension should be accepted")
        .with_problem_policy(policy);

    let json =
        serde_json::to_value(response.problem_details()).expect("Problem Details should serialize");

    assert_eq!(json["debug_state"]["validator"], "email");
    assert_eq!(json["debug_state"]["stage"], 3);
}

#[test]
fn reserved_extension_names_are_rejected() {
    use diagprint_axum::ProblemExtensionError;

    let error = DiagnosticResponse::new(StatusCode::BAD_REQUEST, test_diagnostic())
        .into_problem_details()
        .with_extension("status", serde_json::json!(999))
        .expect_err("reserved extension name must be rejected");

    assert_eq!(
        error,
        ProblemExtensionError::ReservedName("status".to_owned())
    );
}

#[test]
fn invalid_extension_names_are_rejected() {
    use diagprint_axum::ProblemExtensionError;

    for name in ["x", "1debug", "debug-state"] {
        let error = DiagnosticResponse::new(StatusCode::BAD_REQUEST, test_diagnostic())
            .into_problem_details()
            .with_extension(name, serde_json::json!(true))
            .expect_err("invalid extension name must be rejected");

        assert_eq!(error, ProblemExtensionError::InvalidName(name.to_owned()));
    }
}

#[test]
fn duplicate_public_extension_keys_are_rejected() {
    use diagprint_axum::ProblemExtensionError;

    let response = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .into_problem_details()
        .with_extension("errors", serde_json::json!([1]))
        .expect("first extension should be accepted");

    let error = response
        .with_extension("errors", serde_json::json!([2]))
        .expect_err("duplicate extension must be rejected");

    assert_eq!(
        error,
        ProblemExtensionError::DuplicateKey("errors".to_owned())
    );
}

#[test]
fn duplicate_keys_are_rejected_across_public_and_internal_extensions() {
    use diagprint_axum::ProblemExtensionError;

    let response = DiagnosticResponse::new(StatusCode::UNPROCESSABLE_ENTITY, test_diagnostic())
        .into_problem_details()
        .with_extension("validation_state", serde_json::json!("public"))
        .expect("public extension should be accepted");

    let error = response
        .with_internal_extension("validation_state", serde_json::json!("internal"))
        .expect_err("cross-boundary duplicate must be rejected");

    assert_eq!(
        error,
        ProblemExtensionError::DuplicateKey("validation_state".to_owned())
    );
}

#[test]
fn problem_details_response_remains_compact_for_handler_results() {
    assert!(
        std::mem::size_of::<diagprint_axum::ProblemDetailsResponse>() <= 128,
        "ProblemDetailsResponse grew too large for ergonomic Result error usage"
    );
}
