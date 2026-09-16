use crate::{
    ApplicationError, ApplicationErrorExt, AsyncDiagnosticEmissionExt, AsyncEmission,
    ProblemDetailsResponse, RequestContext,
};
use diagprint::Reporter;
use diagprint_async::AsyncDiagnosticSink;
use std::{fmt, sync::Arc};

/// Cloneable application-level state for bounded asynchronous diagnostic
/// submission.
///
/// `AsyncDiagnosticState` groups:
///
/// - a [`Reporter`] used to construct internal diagnostics;
/// - one shared [`AsyncDiagnosticSink`] used for explicit bounded submission.
///
/// The state does not create an async sink, queue, or worker. Applications
/// construct the [`AsyncDiagnosticSink`] explicitly and decide its capacity and
/// backpressure policy before placing it in application state.
///
/// The state also does not own graceful shutdown. Applications remain
/// responsible for flushing and shutting down the async sink at the
/// appropriate lifecycle boundary.
///
/// Cloning this type clones the reporter handle and [`Arc`] pointing to the
/// existing async sink. It does not create another queue or worker.
#[derive(Clone)]
pub struct AsyncDiagnosticState {
    reporter: Reporter,
    sink: Arc<AsyncDiagnosticSink>,
}

impl AsyncDiagnosticState {
    /// Creates async diagnostic application state from a reporter and shared
    /// async diagnostic sink.
    pub fn new(reporter: Reporter, sink: Arc<AsyncDiagnosticSink>) -> Self {
        Self { reporter, sink }
    }

    /// Creates async diagnostic application state and places `sink` into a new
    /// shared [`Arc`].
    ///
    /// Applications that require ownership of the sink for
    /// [`AsyncDiagnosticSink::shutdown`] should normally retain an external
    /// [`Arc`] handle and recover ownership after all application-state clones
    /// have been dropped.
    pub fn with_sink(reporter: Reporter, sink: AsyncDiagnosticSink) -> Self {
        Self::new(reporter, Arc::new(sink))
    }

    /// Returns the reporter used to construct application diagnostics.
    pub const fn reporter(&self) -> &Reporter {
        &self.reporter
    }

    /// Returns the configured bounded asynchronous diagnostic sink.
    pub fn sink(&self) -> &AsyncDiagnosticSink {
        self.sink.as_ref()
    }

    /// Returns a clone of the shared async sink handle.
    pub fn shared_sink(&self) -> Arc<AsyncDiagnosticSink> {
        Arc::clone(&self.sink)
    }

    /// Adapts an application error into correlated RFC 9457 Problem Details
    /// and performs one explicit bounded asynchronous submission attempt.
    ///
    /// This method:
    ///
    /// - constructs the internal diagnostic through [`ApplicationError`];
    /// - adds privacy-safe [`RequestContext`] correlation;
    /// - creates the application's Problem Details response;
    /// - submits the complete internal diagnostic through the existing
    ///   [`AsyncDiagnosticSink`].
    ///
    /// The resulting [`AsyncEmission`] retains whether submission was
    /// enqueued, deliberately dropped, or failed.
    ///
    /// An enqueued outcome means queue acceptance only. It does not mean sink
    /// delivery, persistence, or flush has completed.
    ///
    /// This method does not create a queue, spawn a per-request worker, retry
    /// submission, flush the sink, or shut the sink down.
    pub async fn emit_problem<E>(
        &self,
        error: &E,
        context: &RequestContext,
    ) -> AsyncEmission<ProblemDetailsResponse>
    where
        E: ApplicationError + ?Sized,
    {
        error
            .to_problem_response_with_context(&self.reporter, context)
            .emit_to_async(self.sink())
            .await
    }
}

impl fmt::Debug for AsyncDiagnosticState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AsyncDiagnosticState")
            .field("reporter", &self.reporter)
            .field("sink", &"<diagprint_async::AsyncDiagnosticSink>")
            .finish()
    }
}
