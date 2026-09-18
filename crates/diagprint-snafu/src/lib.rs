//! Structured SNAFU interoperability for diagprint.
//!
//! `diagprint-snafu` keeps application-owned SNAFU errors typed while attaching
//! stable diagnostic identity, reports, source-chain graph evidence, optional
//! privacy/backtrace policy, and strong Whatever/WhateverLocal context.
//!
//! The default path omits backtrace text. Unmapped source text is preserved
//! unless an explicit `SnafuCaptureProfile` requests redaction.

mod bridge;
mod captured;
mod ext;
mod metadata;
mod policy;
mod whatever;

pub use bridge::{
    DefaultSnafuErrorMapper, SnafuBridge, SnafuBridgeError, SnafuBridgeOutput, SnafuDiagnostic,
    SnafuErrorMapper, SnafuErrorView, SnafuMapperFn, snafu_mapper,
};
pub use captured::CapturedSnafuError;
pub use ext::{DiagprintOptionExt, DiagprintResultExt};

#[cfg(feature = "futures")]
pub use ext::{DiagprintTryFutureExt, DiagprintTryStreamExt};

pub use metadata::{SnafuCode, SnafuDiagnosticMetadata, SnafuIdentity, WhateverDiagnosticContext};
pub use policy::{SnafuBacktracePolicy, SnafuCaptureProfile, SnafuTextPolicy};

pub mod prelude {
    pub use crate::{DiagprintOptionExt as _, DiagprintResultExt as _};

    #[cfg(feature = "futures")]
    pub use crate::{DiagprintTryFutureExt as _, DiagprintTryStreamExt as _};
}
