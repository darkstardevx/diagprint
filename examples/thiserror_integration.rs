use diagprint::{
    Applicability, DiagnosticErrorExt, DiagnosticMetadata, Reporter, Severity, Suggestion,
};
use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
enum ConfigError {
    #[error("failed to read configuration `{path}`")]
    Read {
        path: PathBuf,

        #[source]
        source: io::Error,
    },

    #[error("unsupported configuration profile `{profile}`")]
    UnsupportedProfile { profile: String },
}

impl DiagnosticMetadata for ConfigError {
    fn diagnostic_severity(&self) -> Severity {
        match self {
            Self::Read { .. } => Severity::Error,
            Self::UnsupportedProfile { .. } => Severity::Warning,
        }
    }

    fn diagnostic_code(&self) -> Option<String> {
        Some(
            match self {
                Self::Read { .. } => "CONFIG-READ",
                Self::UnsupportedProfile { .. } => "CONFIG-PROFILE",
            }
            .into(),
        )
    }

    fn diagnostic_help(&self) -> Option<String> {
        Some(match self {
            Self::Read { path, .. } => {
                format!("Verify that `{}` exists and is readable.", path.display())
            }

            Self::UnsupportedProfile { .. } => {
                "Use one of the application's supported profile names.".into()
            }
        })
    }

    fn diagnostic_notes(&self) -> Vec<String> {
        match self {
            Self::Read { .. } => vec!["Typed error metadata was supplied by ConfigError.".into()],

            Self::UnsupportedProfile { .. } => {
                vec!["This diagnostic was enriched without parsing the error's \
                 formatted display text."
                    .into()]
            }
        }
    }

    fn diagnostic_suggestions(&self) -> Vec<Suggestion> {
        match self {
            Self::UnsupportedProfile { .. } => {
                vec![Suggestion::new("Select a supported configuration profile")
                    .explanation(
                        "The requested profile is not recognized by the \
                         application.",
                    )
                    .applicability(Applicability::Manual)]
            }

            Self::Read { .. } => Vec::new(),
        }
    }
}

fn main() -> diagprint::Result<()> {
    let reporter = Reporter::builder()
        .application("diagprint-thiserror")
        .width(88)
        .build()?;

    let read_error = ConfigError::Read {
        path: "/etc/cyberdeck/config.toml".into(),
        source: io::Error::new(io::ErrorKind::NotFound, "configuration file does not exist"),
    };

    let profile_error = ConfigError::UnsupportedProfile {
        profile: "quantum-overdrive".into(),
    };

    for error in [&read_error, &profile_error] {
        let diagnostic = error.to_diagprint(&reporter);
        reporter.emit(&diagnostic)?;
    }

    Ok(())
}
