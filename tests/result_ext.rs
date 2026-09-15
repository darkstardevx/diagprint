use diagprint::{DiagnosticMetadata, Reporter, ResultDiagnosticExt, Severity};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("typed failure")]
struct TypedFailure;

impl DiagnosticMetadata for TypedFailure {
    fn diagnostic_code(&self) -> Option<String> {
        Some("TEST-001".into())
    }

    fn diagnostic_severity(&self) -> Severity {
        Severity::Error
    }

    fn diagnostic_help(&self) -> Option<String> {
        Some("fix the input".into())
    }
}

#[test]
fn result_maps_typed_error_to_diagnostic() {
    let reporter = Reporter::builder()
        .application("result-test")
        .build()
        .unwrap();

    let result: std::result::Result<(), TypedFailure> = Err(TypedFailure);

    let diagnostic = result.into_diagnostic(&reporter).unwrap_err();

    assert_eq!(diagnostic.code.as_deref(), Some("TEST-001"));

    assert_eq!(diagnostic.help.as_deref(), Some("fix the input"));
}

#[test]
fn result_can_capture_diagnostic() {
    let reporter = Reporter::builder()
        .application("result-test")
        .source("memory://result.rs", "bad();\n")
        .build()
        .unwrap();

    let result: std::result::Result<(), TypedFailure> = Err(TypedFailure);

    let captured = result.into_captured_diagnostic(&reporter).unwrap_err();

    assert_eq!(captured.diagnostic().code.as_deref(), Some("TEST-001"));
}
