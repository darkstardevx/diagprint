use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
    middleware,
    routing::get,
};
use diagprint::{Diagnostic, Reporter};
use diagprint_axum::{
    ApplicationError, ApplicationErrorExt, ProblemDetailsResponse, REQUEST_ID_HEADER,
    RequestContext, request_context_middleware,
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::{error::Error, fmt};
use tower::ServiceExt;

#[derive(Debug)]
struct OrderConflict;

impl fmt::Display for OrderConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("private order conflict: inventory reservation 8472")
    }
}

impl Error for OrderConflict {}

impl ApplicationError for OrderConflict {
    fn http_status(&self) -> StatusCode {
        StatusCode::CONFLICT
    }

    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic {
        reporter
            .warning("private order conflict: inventory reservation 8472")
            .code("orders.private_conflict")
            .attribute("inventory.reservation", "8472")
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-axum-real-world-test")
        .build()
        .expect("test reporter should build")
}

async fn order_handler(context: RequestContext) -> ProblemDetailsResponse {
    OrderConflict.to_problem_response_with_context(&reporter(), &context)
}

fn application() -> Router {
    Router::new()
        .route("/orders/{id}", get(order_handler))
        .route_layer(middleware::from_fn(request_context_middleware))
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect")
        .to_bytes();

    serde_json::from_slice(&body).expect("response body should contain JSON")
}

#[tokio::test]
async fn real_router_preserves_request_correlation_and_problem_privacy() {
    let response = application()
        .oneshot(
            Request::builder()
                .uri("/orders/42?token=super-secret")
                .header(REQUEST_ID_HEADER, "request-order-42")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::CONFLICT);

    assert_eq!(
        response
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some("request-order-42")
    );

    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json")
    );

    let problem = response_json(response).await;
    let serialized = serde_json::to_string(&problem).expect("problem document should serialize");

    assert_eq!(problem["status"], 409);
    assert_eq!(problem["request_id"], "request-order-42");

    assert_ne!(
        problem["request_id"], problem["report_id"],
        "request identity and diagnostic report identity must remain distinct"
    );

    assert_eq!(problem["detail"], "Request could not be completed.");

    assert!(
        problem.get("code").is_none(),
        "private diagnostic code must remain redacted"
    );

    for forbidden in [
        "private order conflict",
        "orders.private_conflict",
        "inventory.reservation",
        "8472",
        "/orders/42",
        "super-secret",
        "token=",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "client response leaked forbidden value: {forbidden}"
        );
    }
}

#[tokio::test]
async fn generated_request_id_is_shared_by_header_and_problem_document() {
    let response = application()
        .oneshot(
            Request::builder()
                .uri("/orders/7")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    let header_request_id = response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .expect("response should contain generated request ID")
        .to_owned();

    let problem = response_json(response).await;

    assert_eq!(problem["request_id"], header_request_id);
}
