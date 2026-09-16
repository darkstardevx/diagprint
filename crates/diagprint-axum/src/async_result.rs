use crate::{
    ApplicationError, ApplicationErrorExt, AsyncDiagnosticEmissionExt, AsyncDiagnosticState,
    AsyncEmission, ProblemDetailsResponse, RequestContext,
};
use std::future::Future;

/// Handler result whose error side contains an explicit bounded asynchronous
/// Problem Details submission outcome.
///
/// `AsyncEmissionOutcome::Enqueued` means queue acceptance, not completed
/// diagnostic delivery.
pub type AsyncEmittedProblemResult<T> = Result<T, AsyncEmission<ProblemDetailsResponse>>;

/// Explicit bounded asynchronous diagnostic submission for application/domain
/// results.
///
/// This extension is implemented for `Result<T, E>` when `E` implements
/// [`ApplicationError`].
///
/// Successful values pass through unchanged and perform no diagnostic
/// submission.
///
/// Error values are converted into correlated Problem Details synchronously
/// when this method is called. The returned future therefore does not retain
/// the application error across its asynchronous submission boundary.
///
/// This avoids imposing `Send` or `Sync` on `E` merely because diagnostic
/// delivery is asynchronous.
///
/// The successful value must be `Send` because it may be carried by the
/// returned future across an `.await`.
///
/// Queue capacity, backpressure, flushing, and shutdown remain owned by the
/// configured async sink and application lifecycle.
pub trait AsyncApplicationResultExt: Sized {
    /// Successful value carried by this result-like value.
    type Value;

    /// Passes an `Ok` value through unchanged or explicitly submits an `Err`
    /// application diagnostic through the configured bounded async sink.
    ///
    /// Application-error adaptation happens before the returned future crosses
    /// its asynchronous submission boundary.
    fn emit_problem_async<'a>(
        self,
        state: &'a AsyncDiagnosticState,
        context: &RequestContext,
    ) -> impl Future<Output = AsyncEmittedProblemResult<Self::Value>> + Send + 'a
    where
        Self::Value: Send + 'a;
}

impl<T, E> AsyncApplicationResultExt for Result<T, E>
where
    E: ApplicationError,
{
    type Value = T;

    fn emit_problem_async<'a>(
        self,
        state: &'a AsyncDiagnosticState,
        context: &RequestContext,
    ) -> impl Future<Output = AsyncEmittedProblemResult<Self::Value>> + Send + 'a
    where
        T: Send + 'a,
    {
        // Prepare the HTTP/diagnostic response synchronously.
        //
        // This intentionally consumes E before the async block is created, so
        // the returned Send future never carries E or &E across an await.
        let prepared =
            self.map_err(|error| error.to_problem_response_with_context(state.reporter(), context));

        async move {
            match prepared {
                Ok(value) => Ok(value),

                Err(response) => Err(response.emit_to_async(state.sink()).await),
            }
        }
    }
}
