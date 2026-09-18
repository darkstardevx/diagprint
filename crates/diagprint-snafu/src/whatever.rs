use crate::{CapturedSnafuError, SnafuBridge, SnafuCaptureProfile, WhateverDiagnosticContext};
use diagprint::Reporter;
use snafu::{FromString, Whatever, WhateverLocal};

#[track_caller]
pub(crate) fn whatever_with_source<E>(
    source: E,
    reporter: &Reporter,
    context: WhateverDiagnosticContext,
) -> CapturedSnafuError<Whatever>
where
    E: Into<<Whatever as FromString>::Source>,
{
    let (message, metadata) = context.into_parts();
    let error = <Whatever as FromString>::with_source(source.into(), message);
    let capture = SnafuBridge::new().convert_with_metadata(
        &error,
        reporter,
        metadata,
        &SnafuCaptureProfile::default(),
    );
    CapturedSnafuError::new(error, capture)
}

#[track_caller]
pub(crate) fn whatever_without_source(
    reporter: &Reporter,
    context: WhateverDiagnosticContext,
) -> CapturedSnafuError<Whatever> {
    let (message, metadata) = context.into_parts();
    let error = <Whatever as FromString>::without_source(message);
    let capture = SnafuBridge::new().convert_with_metadata(
        &error,
        reporter,
        metadata,
        &SnafuCaptureProfile::default(),
    );
    CapturedSnafuError::new(error, capture)
}

#[track_caller]
pub(crate) fn whatever_local_with_source<E>(
    source: E,
    reporter: &Reporter,
    context: WhateverDiagnosticContext,
) -> CapturedSnafuError<WhateverLocal>
where
    E: Into<<WhateverLocal as FromString>::Source>,
{
    let (message, metadata) = context.into_parts();
    let error = <WhateverLocal as FromString>::with_source(source.into(), message);
    let capture = SnafuBridge::new().convert_with_metadata(
        &error,
        reporter,
        metadata,
        &SnafuCaptureProfile::default(),
    );
    CapturedSnafuError::new(error, capture)
}

#[track_caller]
pub(crate) fn whatever_local_without_source(
    reporter: &Reporter,
    context: WhateverDiagnosticContext,
) -> CapturedSnafuError<WhateverLocal> {
    let (message, metadata) = context.into_parts();
    let error = <WhateverLocal as FromString>::without_source(message);
    let capture = SnafuBridge::new().convert_with_metadata(
        &error,
        reporter,
        metadata,
        &SnafuCaptureProfile::default(),
    );
    CapturedSnafuError::new(error, capture)
}
