//! Privacy-safe Axum integration for the diagprint diagnostics lifecycle.
//!
//! `diagprint-axum` adapts structured diagnostics and Axum request failures
//! without adding Axum or an async runtime dependency to diagprint core.
//!
//! ## Explicit diagnostic emission
//!
//! Response construction and HTTP adaptation remain side-effect free.
//! Applications that want sink delivery must opt in with
//! `DiagnosticEmissionExt::emit_to`.
//!
//! Explicit emission performs one synchronous `diagprint::DiagnosticSink::emit`
//! attempt. It does not automatically flush, retry, queue, persist, or start
//! background work.
//!
//! The sink receives the complete internal diagnostic while the HTTP response
//! continues to follow its existing privacy policy. Sink failures are retained
//! as `EmissionOutcome::Failed` response metadata and do not replace the HTTP
//! response.
//!
//! HTTP responses are treated as an externalization boundary. Internal
//! diagnostic messages and codes are therefore redacted by default.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod application;
mod context;
mod emission;
mod problem;
mod rejection;
mod response;

pub use application::{ApplicationError, ApplicationErrorExt};
pub use context::{
    InvalidRequestId, MAX_REQUEST_ID_LEN, REQUEST_ID_HEADER, RequestContext, RequestId,
    request_context_middleware,
};
pub use emission::{DiagnosticEmissionExt, Emission, EmissionOutcome};
pub use problem::{
    ABOUT_BLANK, PROBLEM_JSON_MEDIA_TYPE, ProblemDetails, ProblemDetailsPolicy,
    ProblemDetailsResponse, ProblemDetailsResponseExt, ProblemExtensionError,
};
pub use rejection::{AxumRejectionExt, RejectionKind};
pub use response::{
    ClientErrorBody, ClientErrorEnvelope, DiagnosticResponse, DiagnosticResponseExt,
    DiagnosticResult, ResponsePolicy,
};
