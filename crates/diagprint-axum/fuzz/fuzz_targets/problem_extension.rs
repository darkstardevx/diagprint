#![no_main]

use axum::http::StatusCode;
use diagprint::Reporter;
use diagprint_axum::{DiagnosticResponse, ProblemDetailsResponseExt};
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

const RESERVED: &[&str] = &[
    "type",
    "title",
    "status",
    "detail",
    "instance",
    "report_id",
    "request_id",
    "code",
];

fn expected(name: &str) -> bool {
    let bytes = name.as_bytes();

    bytes.len() >= 3
        && bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && !RESERVED.contains(&name)
}

fuzz_target!(|data: &[u8]| {
    let name = String::from_utf8_lossy(data);

    let reporter = Reporter::builder()
        .application("diagprint-axum-fuzz")
        .build()
        .expect("fuzz reporter should build");

    let response = DiagnosticResponse::new(
        StatusCode::BAD_REQUEST,
        reporter.warning("internal fuzz diagnostic"),
    )
    .into_problem_details();

    let result = response.with_extension(name.to_string(), Value::Bool(true));

    assert_eq!(result.is_ok(), expected(&name),);
});
