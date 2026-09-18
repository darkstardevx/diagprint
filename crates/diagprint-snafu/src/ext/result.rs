use crate::{
    CapturedSnafuError, SnafuBridge, SnafuDiagnostic, SnafuErrorMapper, WhateverDiagnosticContext,
    whatever::{whatever_local_with_source, whatever_with_source},
};
use diagprint::Reporter;
use snafu::{ErrorCompat, FromString, IntoError, Whatever, WhateverLocal};
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

    fn diagprint_whatever_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<Whatever>>
    where
        E: Into<<Whatever as FromString>::Source>;

    fn diagprint_with_whatever_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<Whatever>>
    where
        E: Into<<Whatever as FromString>::Source>,
        F: FnOnce(&mut E) -> WhateverDiagnosticContext;

    fn diagprint_whatever_local_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>
    where
        E: Into<<WhateverLocal as FromString>::Source>;

    fn diagprint_with_whatever_local_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>
    where
        E: Into<<WhateverLocal as FromString>::Source>,
        F: FnOnce(&mut E) -> WhateverDiagnosticContext;
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

    #[track_caller]
    fn diagprint_whatever_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<Whatever>>
    where
        E: Into<<Whatever as FromString>::Source>,
    {
        self.map_err(|error| whatever_with_source(error, reporter, context))
    }

    #[track_caller]
    fn diagprint_with_whatever_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<Whatever>>
    where
        E: Into<<Whatever as FromString>::Source>,
        F: FnOnce(&mut E) -> WhateverDiagnosticContext,
    {
        match self {
            Ok(value) => Ok(value),
            Err(mut error) => {
                let context = context(&mut error);
                Err(whatever_with_source(error, reporter, context))
            }
        }
    }

    #[track_caller]
    fn diagprint_whatever_local_context(
        self,
        reporter: &Reporter,
        context: WhateverDiagnosticContext,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>
    where
        E: Into<<WhateverLocal as FromString>::Source>,
    {
        self.map_err(|error| whatever_local_with_source(error, reporter, context))
    }

    #[track_caller]
    fn diagprint_with_whatever_local_context<F>(
        self,
        reporter: &Reporter,
        context: F,
    ) -> Result<T, CapturedSnafuError<WhateverLocal>>
    where
        E: Into<<WhateverLocal as FromString>::Source>,
        F: FnOnce(&mut E) -> WhateverDiagnosticContext,
    {
        match self {
            Ok(value) => Ok(value),
            Err(mut error) => {
                let context = context(&mut error);
                Err(whatever_local_with_source(error, reporter, context))
            }
        }
    }
}
