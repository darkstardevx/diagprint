//! Integration adapters for external Rust error and observability ecosystems.
//!
//! Integrations are feature-gated so applications only compile dependencies
//! they explicitly opt into.

#[cfg(feature = "anyhow")]
mod anyhow;

#[cfg(feature = "anyhow")]
pub use self::anyhow::AnyhowDiagnosticExt;
