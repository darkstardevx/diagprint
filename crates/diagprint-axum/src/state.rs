use crate::{
    ApplicationError, ApplicationErrorExt, DiagnosticEmissionExt, Emission, ProblemDetailsResponse,
    RequestContext,
};
use diagprint::{DiagnosticSink, Reporter};
use std::{fmt, sync::Arc};

/// Cloneable application-level diagnostic state for Axum handlers.
///
/// `DiagnosticState` groups the two values normally shared across HTTP
/// requests:
///
/// - a [`Reporter`] used to construct internal diagnostics;
/// - a shared [`DiagnosticSink`] used only when emission is explicitly
///   requested.
///
/// The state itself does not emit diagnostics automatically. Applications must
/// call [`DiagnosticState::emit_problem`] or use the lower-level explicit
/// emission APIs.
///
/// Cloning this type clones the reporter handle and the sink [`Arc`]. It does
/// not create another sink, queue, worker, or application lifecycle.
#[derive(Clone)]
pub struct DiagnosticState {
    reporter: Reporter,
    sink: Arc<dyn DiagnosticSink>,
}

impl DiagnosticState {
    /// Creates diagnostic application state from a reporter and shared sink.
    pub fn new(reporter: Reporter, sink: Arc<dyn DiagnosticSink>) -> Self {
        Self { reporter, sink }
    }

    /// Creates diagnostic application state and places `sink` into a new
    /// shared [`Arc`].
    pub fn with_sink<S>(reporter: Reporter, sink: S) -> Self
    where
        S: DiagnosticSink + 'static,
    {
        Self::new(reporter, Arc::new(sink))
    }

    /// Returns the reporter used to construct application diagnostics.
    pub const fn reporter(&self) -> &Reporter {
        &self.reporter
    }

    /// Returns the configured diagnostic sink.
    pub fn sink(&self) -> &dyn DiagnosticSink {
        self.sink.as_ref()
    }

    /// Returns a clone of the shared sink handle.
    pub fn shared_sink(&self) -> Arc<dyn DiagnosticSink> {
        Arc::clone(&self.sink)
    }

    /// Adapts an application error into correlated RFC 9457 Problem Details
    /// and performs one explicit synchronous diagnostic emission attempt.
    ///
    /// This method is equivalent to:
    ///
    /// - adapting the error through
    ///   [`ApplicationErrorExt::to_problem_response_with_context`];
    /// - then calling [`DiagnosticEmissionExt::emit_to`] with this state's
    ///   configured sink.
    ///
    /// The sink receives the complete internal diagnostic.
    ///
    /// The HTTP client still receives only information permitted by the
    /// application's response and Problem Details policies.
    ///
    /// Sink failure is retained as emission metadata and does not replace the
    /// resulting HTTP response.
    ///
    /// This method performs exactly one synchronous sink emission attempt. It
    /// does not flush, retry, queue, persist, or start background work.
    pub fn emit_problem<E>(
        &self,
        error: &E,
        context: &RequestContext,
    ) -> Emission<ProblemDetailsResponse>
    where
        E: ApplicationError + ?Sized,
    {
        error
            .to_problem_response_with_context(&self.reporter, context)
            .emit_to(self.sink())
    }
}

impl fmt::Debug for DiagnosticState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DiagnosticState")
            .field("reporter", &self.reporter)
            .field("sink", &"<dyn DiagnosticSink>")
            .finish()
    }
}
