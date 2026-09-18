use crate::{CapturedSnafuError, SnafuBridge, SnafuDiagnostic, SnafuErrorMapper};
use diagprint::Reporter;
use snafu::{ErrorCompat, IntoError};
use std::error::Error as StdError;

pub trait DiagprintResultExt<T, E>: Sized {
    fn diagprint(self, reporter: &Reporter) -> Result<T, CapturedSnafuError<E>>
    where
        E: SnafuDiagnostic;
    fn diagprint_with<M>(self, reporter: &Reporter, mapper: &M) -> Result<T, CapturedSnafuError<E>>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized;
    fn diagprint_context<C, E2>(
        self,
        reporter: &Reporter,
        context: C,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        C: IntoError<E2, Source = E>,
        E2: SnafuDiagnostic;
    fn diagprint_context_with_mapper<C, E2, M>(
        self,
        reporter: &Reporter,
        context: C,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        C: IntoError<E2, Source = E>,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized;
    fn diagprint_with_context<F, C, E2>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        F: FnOnce(&mut E) -> C,
        C: IntoError<E2, Source = E>,
        E2: SnafuDiagnostic;
    fn diagprint_with_context_and_mapper<F, C, E2, M>(
        self,
        reporter: &Reporter,
        context: F,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        F: FnOnce(&mut E) -> C,
        C: IntoError<E2, Source = E>,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized;
}
impl<T, E> DiagprintResultExt<T, E> for Result<T, E> {
    fn diagprint(self, reporter: &Reporter) -> Result<T, CapturedSnafuError<E>>
    where
        E: SnafuDiagnostic,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                let c = SnafuBridge::new().convert(&e, reporter);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }
    fn diagprint_with<M>(self, reporter: &Reporter, mapper: &M) -> Result<T, CapturedSnafuError<E>>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                let c = SnafuBridge::new().convert_with_mapper(&e, reporter, mapper);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }
    #[track_caller]
    fn diagprint_context<C, E2>(
        self,
        reporter: &Reporter,
        context: C,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        C: IntoError<E2, Source = E>,
        E2: SnafuDiagnostic,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                let e = context.into_error(e);
                let c = SnafuBridge::new().convert(&e, reporter);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }
    #[track_caller]
    fn diagprint_context_with_mapper<C, E2, M>(
        self,
        reporter: &Reporter,
        context: C,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        C: IntoError<E2, Source = E>,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                let e = context.into_error(e);
                let c = SnafuBridge::new().convert_with_mapper(&e, reporter, mapper);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }
    #[track_caller]
    fn diagprint_with_context<F, C, E2>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        F: FnOnce(&mut E) -> C,
        C: IntoError<E2, Source = E>,
        E2: SnafuDiagnostic,
    {
        match self {
            Ok(v) => Ok(v),
            Err(mut e) => {
                let ctx = context(&mut e);
                let e = ctx.into_error(e);
                let c = SnafuBridge::new().convert(&e, reporter);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }
    #[track_caller]
    fn diagprint_with_context_and_mapper<F, C, E2, M>(
        self,
        reporter: &Reporter,
        context: F,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E2>>
    where
        F: FnOnce(&mut E) -> C,
        C: IntoError<E2, Source = E>,
        E2: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        match self {
            Ok(v) => Ok(v),
            Err(mut e) => {
                let ctx = context(&mut e);
                let e = ctx.into_error(e);
                let c = SnafuBridge::new().convert_with_mapper(&e, reporter, mapper);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }
}
