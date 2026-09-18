use crate::{CapturedSnafuError, SnafuBridge, SnafuDiagnostic, SnafuErrorMapper};
use diagprint::Reporter;
use futures_util::future::{TryFuture, TryFutureExt as FuturesTryFutureExt};
use snafu::{ErrorCompat, IntoError, futures::TryFutureExt as SnafuTryFutureExt};
use std::{error::Error as StdError, future::Future};

/// diagprint capture and SNAFU context combinators for `TryFuture` values.
///
/// The returned futures remain lazy. Creating a combinator does not poll the
/// source future and therefore does not build diagnostic output.
///
/// SNAFU's implicit async location semantics are preserved: implicit location
/// data created by SNAFU context combinators corresponds to poll/combinator
/// execution, not necessarily to the place where this method was called.
pub trait DiagprintTryFutureExt: TryFuture + Sized {
    fn diagprint<'a>(
        self,
        reporter: &'a Reporter,
    ) -> impl Future<Output = Result<Self::Ok, CapturedSnafuError<Self::Error>>> + 'a
    where
        Self: 'a,
        Self::Error: SnafuDiagnostic,
    {
        FuturesTryFutureExt::map_err(self, move |error| {
            let capture = SnafuBridge::new().convert(&error, reporter);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_with<'a, M>(
        self,
        reporter: &'a Reporter,
        mapper: &'a M,
    ) -> impl Future<Output = Result<Self::Ok, CapturedSnafuError<Self::Error>>> + 'a
    where
        Self: 'a,
        Self::Error: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized + 'a,
    {
        FuturesTryFutureExt::map_err(self, move |error| {
            let capture = SnafuBridge::new().convert_with_mapper(&error, reporter, mapper);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_context<'a, C, E2>(
        self,
        reporter: &'a Reporter,
        context: C,
    ) -> impl Future<Output = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        C: IntoError<E2, Source = Self::Error> + 'a,
        E2: SnafuDiagnostic,
    {
        let contextual = SnafuTryFutureExt::context(self, context);

        FuturesTryFutureExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert(&error, reporter);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_context_with_mapper<'a, C, E2, M>(
        self,
        reporter: &'a Reporter,
        context: C,
        mapper: &'a M,
    ) -> impl Future<Output = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        C: IntoError<E2, Source = Self::Error> + 'a,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized + 'a,
    {
        let contextual = SnafuTryFutureExt::context(self, context);

        FuturesTryFutureExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert_with_mapper(&error, reporter, mapper);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_with_context<'a, F, C, E2>(
        self,
        reporter: &'a Reporter,
        context: F,
    ) -> impl Future<Output = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        F: FnOnce(&mut Self::Error) -> C + 'a,
        C: IntoError<E2, Source = Self::Error> + 'a,
        E2: SnafuDiagnostic,
    {
        let contextual = SnafuTryFutureExt::with_context(self, context);

        FuturesTryFutureExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert(&error, reporter);
            CapturedSnafuError::new(error, capture)
        })
    }

    fn diagprint_with_context_and_mapper<'a, F, C, E2, M>(
        self,
        reporter: &'a Reporter,
        context: F,
        mapper: &'a M,
    ) -> impl Future<Output = Result<Self::Ok, CapturedSnafuError<E2>>> + 'a
    where
        Self: 'a,
        F: FnOnce(&mut Self::Error) -> C + 'a,
        C: IntoError<E2, Source = Self::Error> + 'a,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized + 'a,
    {
        let contextual = SnafuTryFutureExt::with_context(self, context);

        FuturesTryFutureExt::map_err(contextual, move |error| {
            let capture = SnafuBridge::new().convert_with_mapper(&error, reporter, mapper);
            CapturedSnafuError::new(error, capture)
        })
    }
}

impl<Fut> DiagprintTryFutureExt for Fut where Fut: TryFuture {}
