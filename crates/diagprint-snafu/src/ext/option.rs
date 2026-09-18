use crate::{
    CapturedSnafuError, SnafuBridge, SnafuDiagnostic, SnafuErrorMapper, WhateverDiagnosticContext,
    whatever::{whatever_local_without_source, whatever_without_source},
};
use diagprint::Reporter;
use snafu::{ErrorCompat, IntoError, NoneError, Whatever, WhateverLocal};
use std::error::Error as StdError;

pub trait DiagprintOptionExt<T>: Sized {
    fn diagprint_context<C, E>(
        self,
        reporter: &Reporter,
        context: C,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        C: IntoError<E, Source = NoneError>,
        E: SnafuDiagnostic;

    fn diagprint_context_with_mapper<C, E, M>(
        self,
        reporter: &Reporter,
        context: C,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        C: IntoError<E, Source = NoneError>,
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized;

    fn diagprint_with_context<F, C, E>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        F: FnOnce() -> C,
        C: IntoError<E, Source = NoneError>,
        E: SnafuDiagnostic;

    fn diagprint_with_context_and_mapper<F, C, E, M>(
        self,
        reporter: &Reporter,
        context: F,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        F: FnOnce() -> C,
        C: IntoError<E, Source = NoneError>,
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized;

    fn diagprint_whatever_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<Whatever>>;

    fn diagprint_with_whatever_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<Whatever>>
    where
        F: FnOnce() -> WhateverDiagnosticContext;

    fn diagprint_whatever_local_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>;

    fn diagprint_with_whatever_local_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>
    where
        F: FnOnce() -> WhateverDiagnosticContext;
}

impl<T> DiagprintOptionExt<T> for Option<T> {
    #[track_caller]
    fn diagprint_context<C, E>(
        self,
        reporter: &Reporter,
        context: C,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        C: IntoError<E, Source = NoneError>,
        E: SnafuDiagnostic,
    {
        match self {
            Some(v) => Ok(v),
            None => {
                let e = context.into_error(NoneError);
                let c = SnafuBridge::new().convert(&e, reporter);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }

    #[track_caller]
    fn diagprint_context_with_mapper<C, E, M>(
        self,
        reporter: &Reporter,
        context: C,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        C: IntoError<E, Source = NoneError>,
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        match self {
            Some(v) => Ok(v),
            None => {
                let e = context.into_error(NoneError);
                let c = SnafuBridge::new().convert_with_mapper(&e, reporter, mapper);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }

    #[track_caller]
    fn diagprint_with_context<F, C, E>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        F: FnOnce() -> C,
        C: IntoError<E, Source = NoneError>,
        E: SnafuDiagnostic,
    {
        match self {
            Some(v) => Ok(v),
            None => {
                let e = context().into_error(NoneError);
                let c = SnafuBridge::new().convert(&e, reporter);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }

    #[track_caller]
    fn diagprint_with_context_and_mapper<F, C, E, M>(
        self,
        reporter: &Reporter,
        context: F,
        mapper: &M,
    ) -> Result<T, CapturedSnafuError<E>>
    where
        F: FnOnce() -> C,
        C: IntoError<E, Source = NoneError>,
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        match self {
            Some(v) => Ok(v),
            None => {
                let e = context().into_error(NoneError);
                let c = SnafuBridge::new().convert_with_mapper(&e, reporter, mapper);
                Err(CapturedSnafuError::new(e, c))
            }
        }
    }

    #[track_caller]
    fn diagprint_whatever_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<Whatever>> {
        match self {
            Some(value) => Ok(value),
            None => Err(whatever_without_source(reporter, context)),
        }
    }

    #[track_caller]
    fn diagprint_with_whatever_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<Whatever>>
    where
        F: FnOnce() -> WhateverDiagnosticContext,
    {
        match self {
            Some(value) => Ok(value),
            None => Err(whatever_without_source(reporter, context())),
        }
    }

    #[track_caller]
    fn diagprint_whatever_local_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>> {
        match self {
            Some(value) => Ok(value),
            None => Err(whatever_local_without_source(reporter, context)),
        }
    }

    #[track_caller]
    fn diagprint_with_whatever_local_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>
    where
        F: FnOnce() -> WhateverDiagnosticContext,
    {
        match self {
            Some(value) => Ok(value),
            None => Err(whatever_local_without_source(reporter, context())),
        }
    }
}
