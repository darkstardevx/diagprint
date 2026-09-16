use crate::{
    DiagnosticResponse, ProblemDetailsPolicy, ProblemDetailsResponse, ProblemDetailsResponseExt,
    ResponsePolicy,
};
use axum::http::StatusCode;
use diagprint::{Diagnostic, Reporter};
use std::error::Error;

/// Application/domain error that can be adapted into diagprint HTTP responses.
///
/// Implementations explicitly control:
///
/// - the HTTP status;
/// - construction of the internal diagnostic;
/// - the client disclosure policy;
/// - RFC 9457 Problem Details metadata.
///
/// `diagprint-axum` deliberately does not infer HTTP status from diagnostic
/// severity and does not expose [`Error::to_string`] automatically.
pub trait ApplicationError: Error {
    /// Returns the HTTP status associated with this application error.
    fn http_status(&self) -> StatusCode;

    /// Creates the internal structured diagnostic for this error.
    ///
    /// This diagnostic remains subject to [`ResponsePolicy`] before any
    /// information crosses the HTTP response boundary.
    fn to_diagnostic(&self, reporter: &Reporter) -> Diagnostic;

    /// Returns the client-disclosure policy for this error.
    ///
    /// The default remains privacy-safe and redacted.
    fn response_policy(&self) -> ResponsePolicy {
        ResponsePolicy::default()
    }

    /// Returns the RFC 9457 representation policy for this error.
    ///
    /// The default uses `about:blank` and the canonical HTTP status title.
    fn problem_policy(&self) -> ProblemDetailsPolicy {
        ProblemDetailsPolicy::default()
    }
}

/// Convenience adapters available to every [`ApplicationError`].
pub trait ApplicationErrorExt: ApplicationError {
    /// Converts this application error into a privacy-safe diagnostic HTTP
    /// response.
    fn to_diagnostic_response(&self, reporter: &Reporter) -> DiagnosticResponse {
        DiagnosticResponse::new(self.http_status(), self.to_diagnostic(reporter))
            .with_policy(self.response_policy())
    }

    /// Converts this application error into an RFC 9457 Problem Details
    /// response.
    ///
    /// Custom RFC 9457 extension members can be added afterward through
    /// [`ProblemDetailsResponse::with_extension`] or
    /// [`ProblemDetailsResponse::with_internal_extension`].
    fn to_problem_response(&self, reporter: &Reporter) -> ProblemDetailsResponse {
        self.to_diagnostic_response(reporter)
            .into_problem_details()
            .with_problem_policy(self.problem_policy())
    }
}

impl<T> ApplicationErrorExt for T where T: ApplicationError + ?Sized {}
