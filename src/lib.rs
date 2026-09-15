//! diagprint v0.3
mod diagnostic;
pub mod render;
mod reporter;
mod rotation;
mod severity;
pub use diagnostic::{Cause, Diagnostic, Label, SourceLocation};
pub use reporter::{Compression, Reporter, ReporterBuilder};
pub use rotation::{RotationCadence, RotationPolicy, RotationState};
pub use severity::Severity;
pub type Result<T> = std::io::Result<T>;
