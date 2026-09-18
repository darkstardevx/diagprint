//! Structured SNAFU interoperability for diagprint.
//!
//! E2A focuses on custom `#[derive(snafu::Snafu)]` errors and synchronous
//! `Result` / `Option` extension traits. The original concrete error is always
//! retained even if diagnostic capture itself fails.

mod bridge;
mod captured;
mod ext;
mod metadata;

pub use bridge::{
    DefaultSnafuErrorMapper, SnafuBridge, SnafuBridgeError, SnafuBridgeOutput, SnafuDiagnostic,
    SnafuErrorMapper, SnafuErrorView,
};
pub use captured::CapturedSnafuError;
pub use ext::{DiagprintOptionExt, DiagprintResultExt};

#[cfg(feature = "futures")]
pub use ext::{DiagprintTryFutureExt, DiagprintTryStreamExt};
pub use metadata::{SnafuCode, SnafuDiagnosticMetadata, SnafuIdentity, WhateverDiagnosticContext};

pub mod prelude {
    pub use crate::{DiagprintOptionExt as _, DiagprintResultExt as _};

    #[cfg(feature = "futures")]
    pub use crate::{DiagprintTryFutureExt as _, DiagprintTryStreamExt as _};
}
