//! Integration adapters for external Rust error, diagnostic, and
//! observability ecosystems.
//!
//! Integrations are feature-gated so applications only compile dependencies
//! they explicitly opt into.

#[cfg(feature = "anyhow")]
mod anyhow;

#[cfg(feature = "codespan-reporting")]
mod codespan;

#[cfg(feature = "miette")]
mod miette;

#[cfg(feature = "tracing")]
mod tracing;

#[cfg(feature = "anyhow")]
pub use self::anyhow::AnyhowDiagnosticExt;

#[cfg(feature = "codespan-reporting")]
pub use self::codespan::CodespanDiagnosticExt;

#[cfg(feature = "miette")]
pub use self::miette::{MietteDiagnosticExt, MietteDiagnosticTree, MietteReportExt};

#[cfg(feature = "tracing")]
pub use self::tracing::TracingLayer;
