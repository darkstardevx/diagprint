use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::get,
};
use diagprint::Reporter;
use diagprint_axum::{
    DiagnosticResponse, PROBLEM_JSON_MEDIA_TYPE, ProblemDetailsPolicy, ProblemDetailsResponse,
    ProblemDetailsResponseExt, ProblemExtensionError, REQUEST_ID_HEADER, RequestContext, RequestId,
    request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const PRIVATE_MESSAGE: &str = "database password=correct-horse-battery-staple";
const PRIVATE_CODE: &str = "database.private_credentials_failure";
const PRIVATE_ATTRIBUTE: &str = "api-key-sk_live_private_attribute";
const PRIVATE_NOTE: &str = "private note: customer SSN 111-22-3333";
const PRIVATE_HELP: &str = "private help: rotate internal root credential";
const PRIVATE_CAUSE: &str = "private cause: postgres://admin:password@db.internal";
const PRIVATE_FILE: &str = "/srv/private/secrets/orders.rs";
const PRIVATE_LABEL: &str = "private label contains implementation detail";
const PRIVATE_INTERNAL_EXTENSION: &str = "internal-extension-secret-value";

fn private_problem() -> ProblemDetailsResponse {
    let reporter = Reporter::builder()
        .application("privacy-contract-private-application")
        .build()
        .expect("security-test reporter should build");

    let diagnostic = reporter
        .error(PRIVATE_MESSAGE)
        .code(PRIVATE_CODE)
        .attribute("security.private_attribute", PRIVATE_ATTRIBUTE)
        .note(PRIVATE_NOTE)
        .help(PRIVATE_HELP)
        .cause(PRIVATE_CAUSE)
        .label(PRIVATE_FILE, 42, Some(7), Some(11), Some(PRIVATE_LABEL));

    DiagnosticResponse::new(StatusCode::INTERNAL_SERVER_ERROR, diagnostic)
        .into_problem_details()
        .with_request_id("public-request-id")
}

fn assert_private_diagnostic_absent(serialized: &str) {
    for forbidden in [
        PRIVATE_MESSAGE,
        PRIVATE_CODE,
        PRIVATE_ATTRIBUTE,
        PRIVATE_NOTE,
        PRIVATE_HELP,
        PRIVATE_CAUSE,
        PRIVATE_FILE,
        PRIVATE_LABEL,
        "privacy-contract-private-application",
        "correct-horse-battery-staple",
        "111-22-3333",
        "postgres://",
        "sk_live",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "client representation leaked private value: {forbidden}"
        );
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    serde_json::from_slice(&body).expect("response should contain valid JSON")
}

#[tokio::test]
async fn default_problem_details_are_fail_closed() {
    let problem = private_problem()
        .with_extension("retry_after", json!(30))
        .expect("public extension should be accepted")
        .with_internal_extension(
            "debug_state",
            json!({
                "secret": PRIVATE_INTERNAL_EXTENSION
            }),
        )
        .expect("internal extension should be accepted");

    let document = problem.problem_details();

    assert_eq!(document.status, StatusCode::INTERNAL_SERVER_ERROR.as_u16());

    assert_eq!(document.detail, "Internal server error.");

    assert_eq!(document.request_id.as_deref(), Some("public-request-id"));

    assert!(document.report_id.is_some());
    assert!(document.code.is_none());

    assert_eq!(document.extensions().get("retry_after"), Some(&json!(30)));

    assert!(!document.extensions().contains_key("debug_state"));

    let serialized = serde_json::to_string(&document).expect("Problem Details should serialize");

    assert_private_diagnostic_absent(&serialized);

    assert!(!serialized.contains(PRIVATE_INTERNAL_EXTENSION));

    let response = problem.into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some(PROBLEM_JSON_MEDIA_TYPE)
    );

    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );

    let body = response_json(response).await;
    let serialized = serde_json::to_string(&body).expect("body should serialize");

    assert_private_diagnostic_absent(&serialized);

    assert!(!serialized.contains(PRIVATE_INTERNAL_EXTENSION));
}

#[test]
fn internal_extension_exposure_does_not_expose_diagnostic_internals() {
    let problem = private_problem()
        .with_internal_extension(
            "debug_state",
            json!({
                "secret": PRIVATE_INTERNAL_EXTENSION
            }),
        )
        .expect("internal extension should be accepted")
        .with_problem_policy(ProblemDetailsPolicy::default().expose_internal_extensions(true));

    let document = problem.problem_details();

    assert_eq!(
        document.extensions().get("debug_state"),
        Some(&json!({
            "secret": PRIVATE_INTERNAL_EXTENSION
        }))
    );

    // Exposing explicitly registered internal extensions must not
    // implicitly expose the underlying diagnostic message, code,
    // attributes, notes, help, causes, labels, or source paths.
    assert_eq!(document.detail, "Internal server error.");

    assert!(document.code.is_none());

    let serialized = serde_json::to_string(&document).expect("Problem Details should serialize");

    assert!(serialized.contains(PRIVATE_INTERNAL_EXTENSION));

    assert_private_diagnostic_absent(&serialized);
}

#[test]
fn reserved_problem_members_cannot_be_overridden() {
    for name in [
        "type",
        "title",
        "status",
        "detail",
        "instance",
        "report_id",
        "request_id",
        "code",
    ] {
        let error = private_problem()
            .with_extension(name, json!("attacker-controlled"))
            .expect_err("reserved member must be rejected");

        match error {
            ProblemExtensionError::ReservedName(actual) => {
                assert_eq!(actual, name);
            }

            other => {
                panic!("unexpected extension error for {name}: {other}");
            }
        }
    }
}

#[test]
fn public_and_internal_extensions_cannot_shadow_each_other() {
    let problem = private_problem()
        .with_extension("errors", json!([]))
        .expect("first extension should be accepted");

    let error = problem
        .with_internal_extension(
            "errors",
            json!({
                "secret": "shadow-attempt"
            }),
        )
        .expect_err("duplicate extension must be rejected");

    assert_eq!(
        error,
        ProblemExtensionError::DuplicateKey("errors".to_owned())
    );
}

#[test]
fn malformed_extension_names_are_rejected() {
    for name in [
        "", "a", "__", "_bad", "9bad", "bad-name", "bad.name", "bad name", "ümlaut",
    ] {
        let error = private_problem()
            .with_extension(name, json!("attacker-controlled"))
            .expect_err("invalid extension must be rejected");

        assert_eq!(error, ProblemExtensionError::InvalidName(name.to_owned()));
    }
}

async fn echo_request_id(context: RequestContext) -> String {
    context.request_id().as_str().to_owned()
}

#[tokio::test]
async fn invalid_inbound_request_id_is_replaced_not_reflected() {
    let attacker_value = "attacker/secret?token=top-secret";

    assert!(
        RequestId::new(attacker_value).is_err(),
        "test value must violate the request-ID contract"
    );

    let app = Router::new()
        .route("/echo", get(echo_request_id))
        .route_layer(middleware::from_fn(request_context_middleware));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/echo?password=hunter2")
                .header(REQUEST_ID_HEADER, attacker_value)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let generated_header = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .expect("response request ID should exist")
        .to_owned();

    assert_ne!(generated_header, attacker_value);

    assert!(RequestId::new(generated_header.clone()).is_ok());

    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let body = String::from_utf8(body.to_vec()).expect("request ID body should be UTF-8");

    assert_eq!(body, generated_header);

    for forbidden in [
        attacker_value,
        "top-secret",
        "hunter2",
        "password=",
        "token=",
    ] {
        assert!(
            !body.contains(forbidden),
            "invalid inbound data was reflected: {forbidden}"
        );
    }
}

async fn requires_context(_context: RequestContext) -> StatusCode {
    StatusCode::OK
}

#[tokio::test]
async fn missing_request_context_fails_closed() {
    let app = Router::new().route("/context-required", get(requires_context));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/context-required")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let body = String::from_utf8_lossy(&body);

    for forbidden in [
        "request context",
        "middleware",
        "configuration",
        "RequestContext",
        "request_context_middleware",
    ] {
        assert!(
            !body.contains(forbidden),
            "wiring details leaked to client: {forbidden}"
        );
    }
}
