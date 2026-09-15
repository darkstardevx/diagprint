#![cfg(feature = "derive")]

use diagprint::{DiagnosticErrorExt, Reporter, Severity, SourceLocation};
use thiserror::Error;

#[derive(Debug, Error, diagprint::Diagnostic)]
#[error("invalid configuration value")]
#[diag(
    code = "CFG-001",
    severity = error,
    help = "use a supported value",
    note = "configuration values are validated before startup"
)]
struct InvalidConfig {
    #[diag(
        primary,
        message = "unsupported value",
        length = length
    )]
    location: SourceLocation,

    length: usize,

    #[diag(secondary, message = "default declared here")]
    default_location: SourceLocation,
}

#[test]
fn derive_generates_structured_metadata() {
    let reporter = Reporter::builder()
        .application("derive-test")
        .build()
        .unwrap();

    let error = InvalidConfig {
        location: SourceLocation {
            file: "config.toml".into(),
            line: 12,
            column: Some(9),
            revision: None,
        },

        length: 5,

        default_location: SourceLocation {
            file: "defaults.toml".into(),
            line: 4,
            column: Some(1),
            revision: None,
        },
    };

    let diagnostic = error.to_diagprint(&reporter);

    assert_eq!(diagnostic.severity, Severity::Error);

    assert_eq!(diagnostic.code.as_deref(), Some("CFG-001"));

    assert_eq!(diagnostic.labels.len(), 2);

    assert_eq!(diagnostic.labels[0].length, Some(5));

    assert_eq!(
        diagnostic.labels[0].message.as_deref(),
        Some("unsupported value")
    );
}
