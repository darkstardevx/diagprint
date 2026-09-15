//! # diagprint
//!
//! Pretty, structured diagnostics and reports for Rust applications.
//!
//! v0.4 introduces diagnostic intelligence: structured suggestions,
//! documentation links, machine-applicable edits, safe fix previews,
//! and optional terminal documentation viewing.

mod diagnostic;
mod fixer;
mod reporter;
mod rotation;
mod severity;
mod suggestion;

#[cfg(feature = "terminal-docs")]
pub mod docs;

pub mod render;

pub use diagnostic::{Cause, Diagnostic, Label, SourceLocation};
pub use fixer::{FixCheck, FixError, FixPreview, FixReport, Fixer};
pub use render::{SeverityTheme, Style, Theme};
pub use reporter::{Compression, Reporter, ReporterBuilder};
pub use rotation::{RotationCadence, RotationPolicy, RotationState};
pub use severity::Severity;
pub use suggestion::{
    Applicability, DocumentationLink, Edit, SuggestedCommand, Suggestion, TextRange,
};

#[cfg(feature = "terminal-docs")]
pub use docs::{TerminalDocError, TerminalDocViewer};

pub type Result<T> = std::io::Result<T>;
