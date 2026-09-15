#![cfg(feature = "derive")]

use diagprint::{Applicability, DiagnosticErrorExt, LabelKind, Reporter, Severity, SourceLocation};
use thiserror::Error;

#[derive(Debug, Error, diagprint::Diagnostic)]
#[diag(
    severity = error,
    note = "configuration diagnostics are validated before startup",
    suggestion(
        title = "Inspect the configuration",
        explanation = "Review the configuration source before retrying",
        applicability = manual
    )
)]
enum ConfigError {
    #[error("missing configuration key")]
    #[diag(
        code = "CFG-101",
        severity = warning,
        help = "add the missing key",
        note = "this key is required",
        suggestion(
            title = "Add the missing key",
            explanation = "Insert the required key with an appropriate value",
            applicability = has_placeholders
        )
    )]
    MissingKey {
        #[diag(primary, message = "key is required here")]
        location: SourceLocation,
    },

    #[error("invalid configuration syntax")]
    #[diag(code = "CFG-102", help = "correct the invalid syntax")]
    InvalidSyntax {
        #[diag(
            primary,
            message = "invalid syntax",
            length = length
        )]
        location: SourceLocation,

        length: usize,

        #[diag(secondary, message = "related declaration")]
        related: SourceLocation,
    },

    #[error("configuration unavailable")]
    #[diag(code = "CFG-103")]
    Unavailable,
}

fn location(file: &str, line: u32, column: u32) -> SourceLocation {
    SourceLocation {
        file: file.into(),
        line,
        column: Some(column),
        revision: None,
    }
}

#[test]
fn enum_variant_overrides_and_inherits_metadata() {
    let reporter = Reporter::builder()
        .application("derive-enum-test")
        .build()
        .unwrap();

    let error = ConfigError::MissingKey {
        location: location("config.toml", 7, 1),
    };

    let diagnostic = error.to_diagprint(&reporter);

    assert_eq!(diagnostic.code.as_deref(), Some("CFG-101"));

    assert_eq!(diagnostic.severity, Severity::Warning);

    assert_eq!(diagnostic.help.as_deref(), Some("add the missing key"));

    assert_eq!(diagnostic.notes.len(), 2);

    assert_eq!(diagnostic.labels.len(), 1);

    assert_eq!(diagnostic.labels[0].kind, LabelKind::Primary);

    assert_eq!(diagnostic.suggestions.len(), 2);

    assert_eq!(
        diagnostic.suggestions[0].applicability,
        Applicability::Manual
    );

    assert_eq!(
        diagnostic.suggestions[1].applicability,
        Applicability::HasPlaceholders
    );

    assert!(!diagnostic.suggestions[1].is_machine_applicable());
}

#[test]
fn enum_named_variant_preserves_multiple_labels() {
    let reporter = Reporter::builder()
        .application("derive-enum-test")
        .build()
        .unwrap();

    let error = ConfigError::InvalidSyntax {
        location: location("config.toml", 10, 5),
        length: 4,
        related: location("defaults.toml", 2, 1),
    };

    let diagnostic = error.to_diagprint(&reporter);

    assert_eq!(diagnostic.code.as_deref(), Some("CFG-102"));

    assert_eq!(diagnostic.severity, Severity::Error);

    assert_eq!(diagnostic.labels.len(), 2);

    assert_eq!(diagnostic.labels[0].length, Some(4));

    assert_eq!(diagnostic.labels[1].kind, LabelKind::Secondary);
}

#[test]
fn unit_variant_is_supported() {
    let reporter = Reporter::builder()
        .application("derive-enum-test")
        .build()
        .unwrap();

    let diagnostic = ConfigError::Unavailable.to_diagprint(&reporter);

    assert_eq!(diagnostic.code.as_deref(), Some("CFG-103"));

    assert_eq!(diagnostic.severity, Severity::Error);

    assert!(diagnostic.labels.is_empty());

    assert_eq!(diagnostic.suggestions.len(), 1);
}
