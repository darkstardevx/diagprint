//! Privacy-safe Axum integration for the diagprint diagnostics lifecycle.
//!
//! `diagprint-axum` adapts structured diagnostics and Axum request failures
//! without adding Axum or an async runtime dependency to diagprint core.
//!
//! HTTP responses are treated as an externalization boundary. Internal
//! diagnostic messages and codes are therefore redacted by default.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod problem;
mod rejection;
mod response;

pub use problem::{
    ABOUT_BLANK, PROBLEM_JSON_MEDIA_TYPE, ProblemDetails, ProblemDetailsPolicy,
    ProblemDetailsResponse, ProblemDetailsResponseExt,
};
pub use rejection::{AxumRejectionExt, RejectionKind};
pub use response::{
    ClientErrorBody, ClientErrorEnvelope, DiagnosticResponse, DiagnosticResponseExt,
    DiagnosticResult, ResponsePolicy,
};
