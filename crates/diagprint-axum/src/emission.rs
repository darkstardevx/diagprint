use crate::{DiagnosticResponse, ProblemDetailsResponse};
use axum::response::{IntoResponse, Response};
use diagprint::{Diagnostic, DiagnosticSink, SinkError, SinkResult};

/// Result of an explicit diagnostic emission attempt.
///
/// Emission failure is retained as response metadata rather than replacing or
/// preventing the HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionOutcome {
    /// The diagnostic was accepted by the configured sink.
    Emitted,

    /// The sink rejected or failed to emit the diagnostic.
    Failed(SinkError),
}

impl EmissionOutcome {
    /// Returns `true` when the diagnostic was emitted successfully.
    pub fn is_emitted(&self) -> bool {
        matches!(self, Self::Emitted)
    }

    /// Returns `true` when the sink returned an error.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns the sink error when emission failed.
    pub fn error(&self) -> Option<&SinkError> {
        match self {
            Self::Emitted => None,
            Self::Failed(error) => Some(error),
        }
    }
}

/// A value paired with the result of an explicit diagnostic emission attempt.
///
/// When the wrapped value implements [`IntoResponse`], converting this wrapper
/// into an Axum response stores the [`EmissionOutcome`] in the response
/// extensions.
///
/// The outcome is internal server-side metadata. It is not serialized into the
/// client response body.
#[derive(Debug, Clone)]
pub struct Emission<T> {
    value: T,
    outcome: EmissionOutcome,
}

impl<T> Emission<T> {
    fn new(value: T, outcome: EmissionOutcome) -> Self {
        Self { value, outcome }
    }

    /// Returns the wrapped value.
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns the result of the emission attempt.
    pub const fn outcome(&self) -> &EmissionOutcome {
        &self.outcome
    }

    /// Consumes the wrapper and returns the wrapped value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consumes the wrapper and returns both the value and emission outcome.
    pub fn into_parts(self) -> (T, EmissionOutcome) {
        (self.value, self.outcome)
    }
}

impl<T> IntoResponse for Emission<T>
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

/// Explicit diagnostic emission for diagprint and diagprint-axum values.
///
/// Calling [`DiagnosticEmissionExt::emit_to`] performs exactly one synchronous
/// call to [`DiagnosticSink::emit`].
///
/// It does not automatically flush, retry, queue, persist, or asynchronously
/// deliver the diagnostic.
pub trait DiagnosticEmissionExt: Sized {
    /// Emits the underlying diagnostic to `sink` and retains the emission
    /// outcome alongside this value.
    ///
    /// Sink failure is represented by [`EmissionOutcome::Failed`] rather than
    /// returned as an error.
    fn emit_to(self, sink: &dyn DiagnosticSink) -> Emission<Self>;
}

impl DiagnosticEmissionExt for Diagnostic {
    fn emit_to(self, sink: &dyn DiagnosticSink) -> Emission<Self> {
        let outcome = emission_outcome(sink.emit(&self));

        Emission::new(self, outcome)
    }
}

impl DiagnosticEmissionExt for DiagnosticResponse {
    fn emit_to(self, sink: &dyn DiagnosticSink) -> Emission<Self> {
        let outcome = emission_outcome(sink.emit(self.diagnostic()));

        Emission::new(self, outcome)
    }
}

impl DiagnosticEmissionExt for ProblemDetailsResponse {
    fn emit_to(self, sink: &dyn DiagnosticSink) -> Emission<Self> {
        let outcome = emission_outcome(sink.emit(self.diagnostic_response().diagnostic()));

        Emission::new(self, outcome)
    }
}

fn emission_outcome(result: SinkResult<()>) -> EmissionOutcome {
    match result {
        Ok(()) => EmissionOutcome::Emitted,
        Err(error) => EmissionOutcome::Failed(error),
    }
}
