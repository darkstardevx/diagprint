use crate::{DiagnosticResponse, ProblemDetailsResponse};
use axum::response::{IntoResponse, Response};
use diagprint::Diagnostic;
use diagprint_async::{AsyncDiagnosticSink, AsyncSinkError, SubmitOutcome};

/// Result of one explicit asynchronous diagnostic submission attempt.
///
/// Submission success means the diagnostic was accepted according to the
/// configured async sink backpressure policy. It does not imply that the
/// underlying synchronous sink has already completed delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncEmissionOutcome {
    /// The diagnostic was accepted into the bounded delivery queue.
    Enqueued,

    /// The configured backpressure policy explicitly dropped the diagnostic.
    Dropped,

    /// The async sink rejected the diagnostic or could not accept it.
    Failed(AsyncSinkError),
}

impl AsyncEmissionOutcome {
    /// Returns `true` when the diagnostic was enqueued.
    pub fn is_enqueued(&self) -> bool {
        matches!(self, Self::Enqueued)
    }

    /// Returns `true` when the diagnostic was deliberately dropped.
    pub fn is_dropped(&self) -> bool {
        matches!(self, Self::Dropped)
    }

    /// Returns `true` when asynchronous submission failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns the asynchronous sink error when submission failed.
    pub fn error(&self) -> Option<&AsyncSinkError> {
        match self {
            Self::Failed(error) => Some(error),
            Self::Enqueued | Self::Dropped => None,
        }
    }
}

/// A value paired with the result of one asynchronous diagnostic submission.
///
/// When the wrapped value implements [`IntoResponse`], converting this wrapper
/// into an Axum response stores [`AsyncEmissionOutcome`] in the response
/// extensions.
///
/// The outcome remains server-side metadata and is never serialized into the
/// client response body.
#[derive(Debug)]
pub struct AsyncEmission<T> {
    value: T,
    outcome: AsyncEmissionOutcome,
}

impl<T> AsyncEmission<T> {
    fn new(value: T, outcome: AsyncEmissionOutcome) -> Self {
        Self { value, outcome }
    }

    /// Returns the wrapped value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns the asynchronous submission outcome.
    pub const fn outcome(&self) -> &AsyncEmissionOutcome {
        &self.outcome
    }

    /// Consumes the wrapper and returns the wrapped value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consumes the wrapper and returns the value and submission outcome.
    pub fn into_parts(self) -> (T, AsyncEmissionOutcome) {
        (self.value, self.outcome)
    }
}

impl<T> IntoResponse for AsyncEmission<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let (value, outcome) = self.into_parts();
        let mut response = value.into_response();

        response.extensions_mut().insert(outcome);

        response
    }
}

/// Explicit asynchronous diagnostic submission for diagprint and
/// diagprint-axum values.
///
/// Calling [`AsyncDiagnosticEmissionExt::emit_to_async`] submits exactly one
/// diagnostic through the supplied [`AsyncDiagnosticSink`].
///
/// Queue capacity and overload behavior remain controlled by the async sink's
/// configured backpressure policy. This adapter does not create its own queue,
/// spawn per-request tasks, flush, retry, or shut down the sink.
#[allow(async_fn_in_trait)]
pub trait AsyncDiagnosticEmissionExt: Sized {
    /// Submits the underlying diagnostic through `sink` and retains the
    /// resulting submission outcome alongside this value.
    ///
    /// Queue rejection, worker failure, and closed-sink errors become
    /// [`AsyncEmissionOutcome::Failed`] rather than replacing the HTTP
    /// response.
    async fn emit_to_async(self, sink: &AsyncDiagnosticSink) -> AsyncEmission<Self>;
}

impl AsyncDiagnosticEmissionExt for Diagnostic {
    async fn emit_to_async(self, sink: &AsyncDiagnosticSink) -> AsyncEmission<Self> {
        let outcome = async_emission_outcome(sink.emit_ref(&self).await);

        AsyncEmission::new(self, outcome)
    }
}

impl AsyncDiagnosticEmissionExt for DiagnosticResponse {
    async fn emit_to_async(self, sink: &AsyncDiagnosticSink) -> AsyncEmission<Self> {
        let outcome = async_emission_outcome(sink.emit_ref(self.diagnostic()).await);

        AsyncEmission::new(self, outcome)
    }
}

impl AsyncDiagnosticEmissionExt for ProblemDetailsResponse {
    async fn emit_to_async(self, sink: &AsyncDiagnosticSink) -> AsyncEmission<Self> {
        let outcome =
            async_emission_outcome(sink.emit_ref(self.diagnostic_response().diagnostic()).await);

        AsyncEmission::new(self, outcome)
    }
}

fn async_emission_outcome(result: Result<SubmitOutcome, AsyncSinkError>) -> AsyncEmissionOutcome {
    match result {
        Ok(SubmitOutcome::Enqueued) => AsyncEmissionOutcome::Enqueued,
        Ok(SubmitOutcome::Dropped) => AsyncEmissionOutcome::Dropped,
        Err(error) => AsyncEmissionOutcome::Failed(error),
    }
}
