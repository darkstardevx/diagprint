use crate::{CapturedDiagnostic, Diagnostic, DiagnosticMetadata, Reporter};
use std::error::Error;

/// Result type whose error is a boxed structured diagnostic.
///
/// Boxing keeps the `Result` representation compact while preserving the full
/// diagnostic model.
pub type DiagnosticResult<T> = std::result::Result<T, Box<Diagnostic>>;

/// Result type whose error is a boxed captured diagnostic.
///
/// The captured diagnostic retains its immutable source snapshot while keeping
/// the `Result` representation compact.
pub type CapturedDiagnosticResult<T> = std::result::Result<T, Box<CapturedDiagnostic>>;

/// Converts typed `Result` errors into native diagprint diagnostics.
pub trait ResultDiagnosticExt<T> {
    /// Converts an error into a boxed structured diagnostic.
    fn into_diagnostic(self, reporter: &Reporter) -> DiagnosticResult<T>;

    /// Converts an error into a boxed diagnostic captured with the reporter's
    /// current immutable source snapshot.
    fn into_captured_diagnostic(self, reporter: &Reporter) -> CapturedDiagnosticResult<T>;
}

impl<T, E> ResultDiagnosticExt<T> for std::result::Result<T, E>
where
    E: Error + DiagnosticMetadata + 'static,
{
    fn into_diagnostic(self, reporter: &Reporter) -> DiagnosticResult<T> {
        self.map_err(|error| Box::new(reporter.diagnostic_from_typed_error(&error)))
    }

    fn into_captured_diagnostic(self, reporter: &Reporter) -> CapturedDiagnosticResult<T> {
        self.map_err(|error| {
            Box::new(reporter.capture(reporter.diagnostic_from_typed_error(&error)))
        })
    }
}
