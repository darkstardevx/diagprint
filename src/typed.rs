use crate::{
    intelligence::{
        cause_chain_from_sources, find_io_error_in_sources, io::suggestion as io_suggestion,
    },
    Diagnostic, Reporter, Severity, Suggestion,
};
use std::error::Error;

/// Optional diagnostic metadata implemented by typed application errors.
///
/// This trait is intentionally independent of any derive crate. Errors built
/// with `thiserror`, handwritten `std::error::Error` implementations, and
/// other error libraries can all implement the same metadata contract.
///
/// The trait supplies structured information that cannot be recovered safely
/// from an error's formatted text.
pub trait DiagnosticMetadata {
    /// Severity to use when converting this error into a diagnostic.
    fn diagnostic_severity(&self) -> Severity {
        Severity::Error
    }

    /// Stable machine-readable/application-readable diagnostic code.
    fn diagnostic_code(&self) -> Option<String> {
        None
    }

    /// Human-readable remediation guidance.
    fn diagnostic_help(&self) -> Option<String> {
        None
    }

    /// Additional contextual notes.
    fn diagnostic_notes(&self) -> Vec<String> {
        Vec::new()
    }

    /// Structured remediation suggestions supplied by the error type.
    fn diagnostic_suggestions(&self) -> Vec<Suggestion> {
        Vec::new()
    }
}

/// Convenience conversion methods for typed errors carrying
/// [`DiagnosticMetadata`].
pub trait DiagnosticErrorExt: Error + DiagnosticMetadata + 'static {
    /// Converts the error into a structured `diagprint` diagnostic.
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        reporter.diagnostic_from_typed_error(self)
    }
}

impl<T> DiagnosticErrorExt for T where T: Error + DiagnosticMetadata + 'static + ?Sized {}

impl Reporter {
    /// Converts a typed error into a structured diagnostic.
    ///
    /// The outer error becomes the diagnostic message. Its
    /// `std::error::Error::source()` chain becomes the diagnostic cause chain.
    /// Metadata supplied by [`DiagnosticMetadata`] is then attached.
    ///
    /// Recognized nested standard-library errors may add conservative
    /// suggestions. Such inferred suggestions are never machine-applicable.
    pub fn diagnostic_from_typed_error<E>(&self, error: &E) -> Diagnostic
    where
        E: Error + DiagnosticMetadata + 'static + ?Sized,
    {
        let mut diagnostic = self.diagnostic(error.diagnostic_severity(), error.to_string());

        if let Some(code) = error.diagnostic_code() {
            diagnostic = diagnostic.code(code);
        }

        if let Some(help) = error.diagnostic_help() {
            diagnostic = diagnostic.help(help);
        }

        for note in error.diagnostic_notes() {
            diagnostic = diagnostic.note(note);
        }

        if let Some(cause) = cause_chain_from_sources(error) {
            diagnostic = diagnostic.cause_chain(cause);
        }

        let mut suggestions = error.diagnostic_suggestions();

        if let Some(io_error) = find_io_error_in_sources(error) {
            let inferred = io_suggestion(io_error);

            let duplicate = suggestions
                .iter()
                .any(|existing| existing.title == inferred.title);

            if !duplicate {
                suggestions.push(inferred);
            }
        }

        diagnostic.suggestions(suggestions)
    }
}
