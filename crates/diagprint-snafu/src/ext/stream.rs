use crate::{CapturedSnafuError, SnafuBridge, SnafuDiagnostic, SnafuErrorMapper};
use diagprint::Reporter;
use futures_util::stream::{Stream, TryStream, TryStreamExt as FuturesTryStreamExt};
use snafu::{ErrorCompat, IntoError, futures::TryStreamExt as SnafuTryStreamExt};
use std::error::Error as StdError;

/// diagprint capture and SNAFU context combinators for `TryStream` values.
///
/// The returned streams remain lazy and preserve item-by-item behavior. Each
/// source error becomes its own `CapturedSnafuError`; this adapter never
/// accumulates a hidden global diagnostic report for the whole stream.
///
/// SNAFU's implicit async location semantics are preserved: implicit location
/// data created by SNAFU context combinators corresponds to poll/combinator
/// execution, not necessarily to the place where this method was called.
pub trait DiagprintTryStreamExt: TryStream + Sized {
    fn diagprint<'a>(
        self,
        reporter: &'a Reporter,
    ) -> impl Stream<Item = Result<Self::Ok, CapturedSnafuError<Self::Error>>> + 'a
    where
        Self: 'a,
        Self::Error: SnafuDiagnostic,
    {
        FuturesTryStreamExt::map_err(self, move |error| {
            let capture = SnafuBridge::new().convert(&error, reporter);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_with<'a, M>(
        self,
        reporter: &'a Reporter,
        mapper: &'a M,
    ) -> impl Stream<Item = Result<Self::Ok, CapturedSnafuError<Self::Error>>> + 'a
    where
        Self: 'a,
        Self::Error: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized + 'a,
    {
        FuturesTryStreamExt::map_err(self, move |error| {
            let capture = SnafuBridge::new().convert_with_mapper(&error, reporter, mapper);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_context<'a, C, E2>(
        self,
        reporter: &'a Reporter,
        context: C,
    ) -> impl Stream<Item = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        C: IntoError<E2, Source = Self::Error> + Clone + 'a,
        E2: SnafuDiagnostic,
    {
        let contextual = SnafuTryStreamExt::context(self, context);

        FuturesTryStreamExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert(&error, reporter);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_context_with_mapper<'a, C, E2, M>(
        self,
        reporter: &'a Reporter,
        context: C,
        mapper: &'a M,
    ) -> impl Stream<Item = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        C: IntoError<E2, Source = Self::Error> + Clone + 'a,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized + 'a,
    {
        let contextual = SnafuTryStreamExt::context(self, context);

        FuturesTryStreamExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert_with_mapper(&error, reporter, mapper);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_with_context<'a, F, C, E2>(
        self,
        reporter: &'a Reporter,
        context: F,
    ) -> impl Stream<Item = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        F: FnMut(&mut Self::Error) -> C + 'a,
        C: IntoError<E2, Source = Self::Error> + 'a,
        E2: SnafuDiagnostic,
    {
        let contextual = SnafuTryStreamExt::with_context(self, context);

        FuturesTryStreamExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert(&error, reporter);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_with_context_and_mapper<'a, F, C, E2, M>(
        self,
        reporter: &'a Reporter,
        context: F,
        mapper: &'a M,
    ) -> impl Stream<Item = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        F: FnMut(&mut Self::Error) -> C + 'a,
        C: IntoError<E2, Source = Self::Error> + 'a,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized + 'a,
    {
        let contextual = SnafuTryStreamExt::with_context(self, context);

        FuturesTryStreamExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert_with_mapper(&error, reporter, mapper);
            CapturedSnafuError::new(error, capture)
        })
    }
}

impl<St> DiagprintTryStreamExt for St where St: TryStream {}
