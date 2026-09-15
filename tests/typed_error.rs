use diagprint::{
    Applicability, DiagnosticErrorExt, DiagnosticMetadata, Reporter, Severity, Suggestion,
};
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
enum DemoError {
    #[error("configuration loading failed")]
    Config {
        #[source]
        source: io::Error,
    },

    #[error("unsupported mode `{mode}`")]
    Mode { mode: String },
}

impl DiagnosticMetadata for DemoError {
    fn diagnostic_severity(&self) -> Severity {
        match self {
            Self::Config { .. } => Severity::Error,
            Self::Mode { .. } => Severity::Warning,
        }
    }

    fn diagnostic_code(&self) -> Option<String> {
        Some(
            match self {
                Self::Config { .. } => "DEMO-CONFIG",
                Self::Mode { .. } => "DEMO-MODE",
            }
            .into(),
        )
    }

    fn diagnostic_help(&self) -> Option<String> {
        Some(match self {
            Self::Config { .. } => "Verify the configuration source.".into(),

            Self::Mode { .. } => "Choose a supported mode.".into(),
        })
    }

    fn diagnostic_notes(&self) -> Vec<String> {
        vec!["typed diagnostic metadata".into()]
    }

    fn diagnostic_suggestions(&self) -> Vec<Suggestion> {
        match self {
            Self::Mode { .. } => {
                vec![Suggestion::new("Select a supported mode").applicability(Applicability::Manual)]
            }

            Self::Config { .. } => Vec::new(),
        }
    }
}

#[test]
fn typed_metadata_is_applied() {
    let reporter = Reporter::builder().build().unwrap();

    let error = DemoError::Mode {
        mode: "quantum".into(),
    };

    let diagnostic = reporter.diagnostic_from_typed_error(&error);

    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("DEMO-MODE"));

    assert_eq!(diagnostic.help.as_deref(), Some("Choose a supported mode."));

    assert_eq!(diagnostic.notes, vec!["typed diagnostic metadata"]);

    assert_eq!(diagnostic.suggestions.len(), 1);

    assert_eq!(diagnostic.suggestions[0].title, "Select a supported mode");
}

#[test]
fn thiserror_source_chain_is_preserved() {
    let reporter = Reporter::builder().build().unwrap();

    let error = DemoError::Config {
        source: io::Error::new(io::ErrorKind::NotFound, "settings file is missing"),
    };

    let diagnostic = reporter.diagnostic_from_typed_error(&error);

    let cause = diagnostic.cause.expect("source cause");

    assert_eq!(cause.message, "settings file is missing");

    assert!(cause.source.is_none());
}

#[test]
fn nested_io_error_receives_inferred_intelligence() {
    let reporter = Reporter::builder().build().unwrap();

    let error = DemoError::Config {
        source: io::Error::new(io::ErrorKind::PermissionDenied, "access denied"),
    };

    let diagnostic = reporter.diagnostic_from_typed_error(&error);

    assert_eq!(diagnostic.suggestions.len(), 1);

    let suggestion = &diagnostic.suggestions[0];

    assert_eq!(suggestion.applicability, Applicability::Manual);

    assert!(suggestion.title.to_ascii_lowercase().contains("permission"));
}

#[test]
fn extension_trait_matches_reporter_conversion() {
    let reporter = Reporter::builder().build().unwrap();

    let error = DemoError::Mode {
        mode: "unknown".into(),
    };

    let via_reporter = reporter.diagnostic_from_typed_error(&error);

    let via_extension = error.to_diagprint(&reporter);

    assert_eq!(via_reporter.message, via_extension.message);

    assert_eq!(via_reporter.code, via_extension.code);

    assert_eq!(via_reporter.severity, via_extension.severity);
}

#[test]
fn inferred_io_suggestion_does_not_become_machine_applicable() {
    let reporter = Reporter::builder().build().unwrap();

    let error = DemoError::Config {
        source: io::Error::new(io::ErrorKind::NotFound, "missing"),
    };

    let diagnostic = error.to_diagprint(&reporter);

    assert!(diagnostic
        .suggestions
        .iter()
        .all(|suggestion| { suggestion.applicability != Applicability::MachineApplicable }));
}
