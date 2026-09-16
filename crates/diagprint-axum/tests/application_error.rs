use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    routing::get,
};
use diagprint::{Diagnostic, Reporter};
use diagprint_axum::{
    ApplicationError, ApplicationErrorExt, ProblemDetailsPolicy, ProblemDetailsResponse,
    ResponsePolicy,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{error::Error, fmt};
use tower::ServiceExt;

#[derive(Debug)]
enum ShopError {
    MissingOrder,
    InvalidEmail,
    DatabaseUnavailable,
}

impl fmt::Display for ShopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOrder => write!(f, "internal order lookup failed"),
            Self::InvalidEmail => write!(f, "email validator rejected input"),
            Self::DatabaseUnavailable => {
                write!(f, "primary database connection secret detail")
            }
        }
    }
}

impl Error for ShopError {}

impl ApplicationError for ShopError {
    fn http_status(&self) -> StatusCode {
        match self {
            Self::MissingOrder => StatusCode::NOT_FOUND,
            Self::InvalidEmail => StatusCode::UNPROCESSABLE_ENTITY,
            Self::DatabaseUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        match self {
            Self::MissingOrder => reporter
                .warning("order was not found")
                .code("shop.order.not_found"),

            Self::InvalidEmail => reporter
                .warning("email validation failed internally")
                .code("shop.validation.email"),

            Self::DatabaseUnavailable => reporter
                .error("primary database connection secret detail")
                .code("shop.database.unavailable"),
        }
    }

    fn problem_policy(&self) -> ProblemDetailsPolicy {
        match self {
            Self::MissingOrder => ProblemDetailsPolicy::default()
                .with_type_uri("https://api.example.com/problems/order-not-found")
                .with_title("Order not found"),

            Self::InvalidEmail => ProblemDetailsPolicy::default()
                .with_type_uri("https://api.example.com/problems/validation")
                .with_title("Validation failed"),

            Self::DatabaseUnavailable => ProblemDetailsPolicy::default()
                .with_type_uri("https://api.example.com/problems/service-unavailable")
                .with_title("Service unavailable"),
        }
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-application-error-test")
        .build()
        .expect("test reporter should build")
}

#[test]
fn application_error_preserves_explicit_http_status() {
    let reporter = reporter();

    let response = ShopError::MissingOrder.to_diagnostic_response(&reporter);

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.diagnostic().code.as_deref(),
        Some("shop.order.not_found")
    );
}

#[test]
fn application_error_problem_response_is_redacted_by_default() {
    let reporter = reporter();

    let problem = ShopError::DatabaseUnavailable
        .to_problem_response(&reporter)
        .problem_details();

    assert_eq!(problem.status, 503);
    assert_eq!(problem.title, "Service unavailable");
    assert_eq!(
        problem.type_uri,
        "https://api.example.com/problems/service-unavailable"
    );

    assert_eq!(problem.detail, "Internal server error.");
    assert_eq!(problem.code, None);

    assert!(!problem.detail.contains("database"));
    assert!(!problem.detail.contains("secret"));
}

#[test]
fn application_error_can_define_problem_type_and_title() {
    let reporter = reporter();

    let problem = ShopError::InvalidEmail
        .to_problem_response(&reporter)
        .problem_details();

    assert_eq!(problem.status, 422);
    assert_eq!(problem.title, "Validation failed");
    assert_eq!(
        problem.type_uri,
        "https://api.example.com/problems/validation"
    );
}

#[test]
fn application_error_can_opt_into_public_diagnostic_fields() {
    #[derive(Debug)]
    struct PublicError;

    impl fmt::Display for PublicError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "public validation failure")
        }
    }

    impl Error for PublicError {}

    impl ApplicationError for PublicError {
        fn http_status(&self) -> StatusCode {
            StatusCode::BAD_REQUEST
        }

        fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
            reporter
                .warning("public validation failure")
                .code("validation.public")
        }

        fn response_policy(&self) -> ResponsePolicy {
            ResponsePolicy::default()
                .expose_diagnostic_message(true)
                .expose_diagnostic_code(true)
        }
    }

    let reporter = reporter();

    let problem = PublicError.to_problem_response(&reporter).problem_details();

    assert_eq!(problem.detail, "public validation failure");
    assert_eq!(problem.code.as_deref(), Some("validation.public"));
}

#[test]
fn custom_problem_extensions_layer_cleanly_on_application_errors() {
    let reporter = reporter();

    let response = ShopError::InvalidEmail
        .to_problem_response(&reporter)
        .with_extension(
            "errors",
            serde_json::json!([
                {
                    "field": "email",
                    "code": "invalid_format"
                }
            ]),
        )
        .expect("validation extension should be accepted");

    let json =
        serde_json::to_value(response.problem_details()).expect("Problem Details should serialize");

    assert_eq!(json["errors"][0]["field"], "email");
    assert_eq!(json["errors"][0]["code"], "invalid_format");
}

#[test]
fn internal_extensions_remain_private_on_application_errors() {
    let reporter = reporter();

    let response = ShopError::DatabaseUnavailable
        .to_problem_response(&reporter)
        .with_internal_extension(
            "database_state",
            serde_json::json!({
                "cluster": "primary",
                "connection_attempt": 4
            }),
        )
        .expect("internal extension should be accepted");

    let json =
        serde_json::to_value(response.problem_details()).expect("Problem Details should serialize");

    assert!(json.get("database_state").is_none());
}

#[test]
fn request_correlation_layers_on_application_errors() {
    let reporter = reporter();

    let problem = ShopError::MissingOrder
        .to_problem_response(&reporter)
        .with_request_id("request-app-error-123")
        .problem_details();

    assert_eq!(problem.request_id.as_deref(), Some("request-app-error-123"));
    assert!(problem.report_id.is_some());
    assert_ne!(problem.request_id, problem.report_id);
}

async fn missing_order_handler() -> Result<StatusCode, ProblemDetailsResponse> {
    let reporter = reporter();

    Err(ShopError::MissingOrder.to_problem_response(&reporter))
}

#[tokio::test]
async fn application_error_adapter_works_in_real_axum_handler() {
    let app = Router::new().route("/orders/42", get(missing_order_handler));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/orders/42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static(
            "application/problem+json"
        ))
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    let json: Value = serde_json::from_slice(&body).expect("body should be valid JSON");

    assert_eq!(
        json["type"],
        "https://api.example.com/problems/order-not-found"
    );
    assert_eq!(json["title"], "Order not found");
    assert_eq!(json["status"], 404);
    assert_eq!(json["detail"], "Request could not be completed.");

    let serialized = String::from_utf8(body.to_vec()).expect("body should be UTF-8");

    assert!(!serialized.contains("internal order lookup failed"));
    assert!(!serialized.contains("shop.order.not_found"));
}
