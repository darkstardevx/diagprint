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
//! ## Application-state ergonomics
//!
//! [`DiagnosticState`] groups a cloneable [`diagprint::Reporter`] with one
//! shared [`diagprint::DiagnosticSink`] for use in Axum application state.
//!
//! Application errors can be converted into correlated Problem Details and
//! explicitly emitted with [`DiagnosticState::emit_problem`].
//!
//! This remains an explicit side effect. Storing a sink in application state
//! does not cause responses, errors, or diagnostics to emit automatically.
//!
////! ## Optional asynchronous delivery
//!
//! The `async-delivery` feature adds an explicit asynchronous submission bridge
//! backed by `diagprint-async`.
//!
//! `AsyncDiagnosticEmissionExt::emit_to_async` submits one diagnostic through
//! an existing bounded `diagprint_async::AsyncDiagnosticSink`.
//!
//! An `Enqueued` outcome means queue acceptance, not completed sink delivery.
//! Queue ownership, backpressure, flushing, and shutdown remain the
//! responsibility of `diagprint-async` and the application lifecycle.
//!
//! The Axum adapter does not create a second queue, spawn a task per request,
//! retry delivery, flush automatically, or shut the sink down.
//!
//! The `async-delivery` feature is disabled by default. Without it,
//! `diagprint-async` is not part of the crate's normal dependency graph.
//!
//! HTTP responses are treated as an externalization boundary. Internal
//! diagnostic messages and codes are therefore redacted by default.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod application;

#[cfg(feature = "async-delivery")]
mod async_emission;

mod context;
mod emission;
mod problem;
mod rejection;
mod response;
mod state;

pub use application::{ApplicationError, ApplicationErrorExt};

#[cfg(feature = "async-delivery")]
pub use async_emission::{AsyncDiagnosticEmissionExt, AsyncEmission, AsyncEmissionOutcome};

pub use context::{
    InvalidRequestId, MAX_REQUEST_ID_LEN, MissingRequestContext, REQUEST_ID_HEADER, RequestContext,
    RequestId, request_context_middleware,
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

pub use state::DiagnosticState;
