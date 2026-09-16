use crate::{ApplicationError, DiagnosticState, Emission, ProblemDetailsResponse, RequestContext};

/// Handler result whose error side is an explicitly emitted RFC 9457 Problem
/// Details response.
///
/// The error wrapper retains the [`crate::EmissionOutcome`] produced by the
/// configured synchronous diagnostic sink.
pub type EmittedProblemResult<T> = Result<T, Emission<ProblemDetailsResponse>>;

/// Explicit synchronous diagnostic emission for application/domain results.
///
/// This extension is implemented for `Result<T, E>` when `E` implements
/// [`ApplicationError`].
///
/// Successful values pass through unchanged and perform no diagnostic
/// emission.
///
/// Error values are adapted through [`DiagnosticState::emit_problem`], which:
///
/// - attaches privacy-safe [`RequestContext`] correlation;
/// - creates RFC 9457 Problem Details;
/// - performs exactly one synchronous diagnostic sink emission attempt;
/// - preserves the emission outcome alongside the HTTP response.
///
/// Calling this method is therefore an explicit side effect. It does not
/// flush, retry, queue, or start background work.
pub trait ApplicationResultExt: Sized {
    /// Successful value carried by this result-like value.
    type Value;

    /// Passes an `Ok` value through unchanged or explicitly emits an `Err`
    /// application diagnostic and converts it into Problem Details.
    fn emit_problem(
        self,
        state: &DiagnosticState,
        context: &RequestContext,
    ) -> EmittedProblemResult<Self::Value>;
}

impl<T, E> ApplicationResultExt for Result<T, E>
where
    E: ApplicationError,
{
    type Value = T;

    fn emit_problem(
        self,
        state: &DiagnosticState,
        context: &RequestContext,
    ) -> EmittedProblemResult<Self::Value> {
        self.map_err(|error| state.emit_problem(&error, context))
    }
}
