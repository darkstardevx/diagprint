//! Integration adapters for external Rust error and observability ecosystems.
//!
//! Integrations are feature-gated so applications only compile dependencies
//! they explicitly opt into.

#[cfg(feature = "anyhow")]
mod anyhow;

#[cfg(feature = "tracing")]
mod tracing;

#[cfg(feature = "anyhow")]
pub use self::anyhow::AnyhowDiagnosticExt;

#[cfg(feature = "tracing")]
pub use self::tracing::TracingLayer;
