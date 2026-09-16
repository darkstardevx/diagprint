use crate::{DiagnosticResponse, ProblemDetailsResponse};
use axum::response::{IntoResponse, Response};
use diagprint::{Diagnostic, DiagnosticSink, SinkError, SinkResult};

/// Result of an explicit diagnostic emission attempt.
///
/// Emission failure is intentionally separate from HTTP response generation.
/// A sink failure therefore never replaces or prevents the client response.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionOutcome {
    /// The configured diagnostic sink accepted the diagnostic.
    Emitted,

    /// The configured diagnostic sink returned an error.
    Failed(SinkError),
}

impl EmissionOutcome {
    /// Returns `true` when the sink accepted the diagnostic.
    pub const fn is_emitted(&self) -> bool {
        matches!(self, Self::Emitted)
    }

    /// Returns `true` when the sink returned an error.
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns the sink error when emission failed.
    pub const fn error(&self) -> Option<&SinkError> {
        match self {
            Self::Emitted => None,
            Self::Failed(error) => Some(error),
        }
    }

    fn from_result(result: SinkResult<()>) -> Self {
        match result {
            Ok(()) => Self::Emitted,
            Err(error) => Self::Failed(error),
        }
    }
}

/// Value paired with the outcome of one explicit diagnostic emission.
///
/// When the wrapped value implements [`IntoResponse`], this wrapper also
/// implements `IntoResponse`. The emission outcome is retained in the
/// resulting Axum response extensions for server-side middleware inspection.
///
/// The emission outcome is never serialized into the client response body.
#[must_use = "return the wrapped response or inspect its emission outcome"]
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

    /// Returns the diagnostic emission outcome.
    pub const fn outcome(&self) -> &EmissionOutcome {
        &self.outcome
    }

    /// Consumes this wrapper and returns the wrapped value.
    pub fn into_value(self) -> T {
        self.value
    }

    /// Consumes this wrapper and returns both the value and emission outcome.
    pub fn into_parts(self) -> (T, EmissionOutcome) {
        (self.value, self.outcome)
    }
}

impl<T> IntoResponse for Emission<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let Self { value, outcome } = self;

        let mut response = value.into_response();

        response.extensions_mut().insert(outcome);

        response
    }
}

/// Explicit diagnostic-emission extension.
///
/// Calling [`Self::emit_to`] is the lifecycle action. Merely constructing a
/// diagnostic or HTTP response never emits anything.
///
/// The supplied [`DiagnosticSink`] receives the full internal diagnostic,
/// including request-correlation attributes when they were attached earlier.
///
/// Sink failures are represented by [`EmissionOutcome::Failed`] and never
/// replace the HTTP response.
pub trait DiagnosticEmissionExt: Sized {
    /// Emits this value's internal diagnostic to `sink`.
    ///
    /// This method performs exactly one sink emission attempt. It does not
    /// automatically flush the sink.
    fn emit_to<S>(self, sink: &S) -> Emission<Self>
    where
        S: DiagnosticSink + ?Sized;
}

impl DiagnosticEmissionExt for Diagnostic {
    fn emit_to<S>(self, sink: &S) -> Emission<Self>
    where
        S: DiagnosticSink + ?Sized,
    {
        let outcome = EmissionOutcome::from_result(sink.emit(&self));

        Emission::new(self, outcome)
    }
}

impl DiagnosticEmissionExt for DiagnosticResponse {
    fn emit_to<S>(self, sink: &S) -> Emission<Self>
    where
        S: DiagnosticSink + ?Sized,
    {
        let outcome = EmissionOutcome::from_result(sink.emit(self.diagnostic()));

        Emission::new(self, outcome)
    }
}

impl DiagnosticEmissionExt for ProblemDetailsResponse {
    fn emit_to<S>(self, sink: &S) -> Emission<Self>
    where
        S: DiagnosticSink + ?Sized,
    {
        let outcome =
            EmissionOutcome::from_result(sink.emit(self.diagnostic_response().diagnostic()));

        Emission::new(self, outcome)
    }
}
