use crate::{
    DiagnosticResponse, ProblemDetailsPolicy, ProblemDetailsResponse, ProblemDetailsResponseExt,
    RequestContext, ResponsePolicy,
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
/// severity and does not expose the error's [`Display`](std::fmt::Display)
/// representation automatically.
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

    /// Converts this application error into a diagnostic response and attaches
    /// privacy-safe HTTP request context to the internal diagnostic.
    fn to_diagnostic_response_with_context(
        &self,
        reporter: &Reporter,
        context: &RequestContext,
    ) -> DiagnosticResponse {
        let diagnostic = context.annotate_diagnostic(self.to_diagnostic(reporter));

        DiagnosticResponse::new(self.http_status(), diagnostic).with_policy(self.response_policy())
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

    /// Converts this application error into RFC 9457 Problem Details while
    /// correlating the HTTP request and internal diagnostic.
    ///
    /// The request ID is:
    ///
    /// - attached to the internal diagnostic as `http.request_id`;
    /// - included in the Problem Details document as `request_id`;
    /// - kept distinct from the diagnostic's `report_id`.
    fn to_problem_response_with_context(
        &self,
        reporter: &Reporter,
        context: &RequestContext,
    ) -> ProblemDetailsResponse {
        self.to_diagnostic_response_with_context(reporter, context)
            .into_problem_details()
            .with_problem_policy(self.problem_policy())
            .with_request_id(context.request_id().as_str())
    }
}

impl<T> ApplicationErrorExt for T where T: ApplicationError + ?Sized {}
