//! # diagprint
//!
//! `diagprint` provides structured diagnostics, rich terminal rendering,
//! persistent reports, and guarded diagnostic fixes for Rust applications.
//!
//! The crate deliberately separates **diagnostic data** from
//! **presentation and mutation**:
//!
//! - [`Diagnostic`] describes what happened.
//! - renderers decide how the diagnostic is displayed.
//! - [`Suggestion`] describes a possible resolution.
//! - [`Fixer`] validates and optionally applies structured text edits.
//!
//! Calling a renderer never modifies source files.
//!
//! ## Quick start
//!
//! ```
//! use diagprint::{Reporter, Severity};
//!
//! # fn main() -> diagprint::Result<()> {
//! let reporter = Reporter::builder()
//!     .application("myapp")
//!     .min_severity(Severity::Info)
//!     .build()?;
//!
//! let diagnostic = reporter
//!     .error("Network initialization failed")
//!     .code("NET-001")
//!     .cause("failed to open network interface")
//!     .help("Check interface permissions and driver state");
//!
//! reporter.emit(&diagnostic)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Diagnostic intelligence
//!
//! v0.4 adds structured suggestions and guarded file edits.
//!
//! A suggestion can carry:
//!
//! - an explanation;
//! - documentation links;
//! - one or more structured edits;
//! - follow-up commands;
//! - an [`Applicability`] classification.
//!
//! ```no_run
//! use diagprint::{
//!     Applicability, DocumentationLink, Edit, Fixer, Reporter,
//!     Suggestion, TextRange,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let reporter = Reporter::builder().build()?;
//!
//! let original =
//!     r#"chrono = { version = "0.4", features = ["clock"] }"#;
//!
//! let replacement =
//!     r#"chrono = { version = "0.4", features = ["clock", "serde"] }"#;
//!
//! let diagnostic = reporter
//!     .error("chrono serde support is missing")
//!     .suggestion(
//!         Suggestion::new("Enable chrono's serde feature")
//!             .explanation(
//!                 "chrono requires its `serde` feature for serde support",
//!             )
//!             .applicability(Applicability::MachineApplicable)
//!             .documentation(
//!                 DocumentationLink::docs_rs(
//!                     "chrono",
//!                     "latest",
//!                     "",
//!                 ),
//!             )
//!             .edit(Edit::replace(
//!                 "Cargo.toml",
//!                 TextRange::new(0, original.len()),
//!                 original,
//!                 replacement,
//!             )),
//!     );
//!
//! // Validate against the current filesystem without making changes.
//! let check = Fixer::new().check(&diagnostic)?;
//!
//! println!(
//!     "{} suggestion(s) currently applicable",
//!     check.applicable_suggestions
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Automatic fix safety
//!
//! `diagprint` does not treat every suggestion as automatically applicable.
//!
//! Automatic editing requires:
//!
//! 1. [`Applicability::MachineApplicable`];
//! 2. at least one structured [`Edit`];
//! 3. valid UTF-8 edit boundaries;
//! 4. non-overlapping edits;
//! 5. the expected original file contents still being present.
//!
//! If the file has changed since the diagnostic was constructed, the edit is
//! rejected instead of guessing.
//!
//! Suggested shell commands are informational only and are never executed by
//! [`Fixer`].
//!
//! ## Terminal documentation
//!
//! The optional `terminal-docs` feature can fetch documentation and display it
//! directly in a terminal.
//!
//! The viewer:
//!
//! - accepts only HTTP and HTTPS URLs;
//! - limits remote document size;
//! - sanitizes terminal control characters;
//! - extracts readable documentation text;
//! - syntax-highlights code examples.
//!
//! The current documentation viewer uses a synchronous HTTP client. Programs
//! using an async runtime should invoke it from an appropriate blocking worker.
//!
//! ## Themes
//!
//! [`Theme`], [`Style`], and [`SeverityTheme`] control terminal presentation.
//! Disabling renderer color remains authoritative regardless of the selected
//! theme.
//!
//! The optional `cybercore` feature maps the Cybercore semantic palette into
//! `diagprint` rather than copying Cybercore color values.
//!
//! ## Report formats
//!
//! `diagprint` supports:
//!
//! - rich terminal output;
//! - plain text;
//! - JSON;
//! - Markdown.
//!
//! File reporting also supports rotation, retention, and optional compression.
//!
//! ## Feature flags
//!
//! - `compression` — gzip and Zstandard report compression.
//! - `cybercore` — Cybercore theme-schema integration.
//! - `terminal-docs` — in-terminal documentation retrieval and syntax
//!   highlighting.
//!
//! All optional features are disabled by default.

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

/// Convenience result type used by `diagprint` reporting operations.
pub type Result<T> = std::io::Result<T>;
