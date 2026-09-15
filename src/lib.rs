//! # diagprint
//!
//! `diagprint` provides structured diagnostics, rich terminal rendering,
//! persistent reports, guarded remediation, compiler-diagnostic ingestion,
//! version-aware documentation intelligence, and Rust ecosystem integrations.
//!
//! The crate deliberately separates diagnostic data from presentation and
//! mutation:
//!
//! - [`Diagnostic`] describes what happened.
//! - [`Suggestion`] describes a possible resolution.
//! - [`Fixer`] validates and applies guarded structured edits.
//! - [`FixPlan`] coordinates transactional multi-file remediation.
//! - [`DocumentationResolver`] resolves documentation without guessing package
//!   versions.
//! - [`DiagnosticMetadata`] lets typed errors supply semantic metadata.
//! - [`CompilerImporter`] consumes structured rustc and Cargo diagnostics.
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
//! Structured suggestions can carry explanations, documentation links,
//! structured edits, advisory follow-up commands, and an [`Applicability`]
//! classification.
//!
//! Automatic editing requires [`Applicability::MachineApplicable`], valid
//! non-overlapping UTF-8 edit ranges, and verification of expected source
//! contents.
//!
//! Suggested shell commands are informational only and are never executed by
//! [`Fixer`] or [`FixPlan`].
//!
//! ## Transactional remediation
//!
//! [`FixPlan`] supports deterministic preconditions, multi-file preparation,
//! rollback-on-error writes, post-apply verification, verification rollback,
//! and optional backups.
//!
//! Verification is declarative and does not execute shell commands.
//!
//! ## Documentation intelligence
//!
//! [`DocumentationResolver`] can build a package/version catalog directly from
//! `Cargo.lock`.
//!
//! If exactly one version of a package is locked, a version-specific docs.rs
//! URL can be produced automatically.
//!
//! If multiple versions are present, resolution fails instead of silently
//! choosing one.
//!
//! Rust-owned documentation can also be pinned to a toolchain release:
//!
//! ```no_run
//! use diagprint::DocumentationResolver;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let resolver = DocumentationResolver::from_cargo_lock("Cargo.lock")?
//!     .rust_version("1.98.0");
//!
//! let serde = resolver.crate_docs(
//!     "serde",
//!     "trait.Deserialize.html",
//! )?;
//!
//! let e0277 = resolver.rust_error("E0277");
//!
//! println!("{}", serde.url);
//! println!("{}", e0277.url);
//! # Ok(())
//! # }
//! ```
//!
//! ## Compiler diagnostics
//!
//! [`CompilerImporter`] consumes structured rustc JSON diagnostics directly,
//! including diagnostics embedded in Cargo `compiler-message` records.
//!
//! It preserves compiler severity, Rust error codes, source spans, notes,
//! help, structured replacements, applicability, and Cargo context.
//!
//! A [`DocumentationResolver`] can be attached to the importer so compiler
//! documentation links follow the same toolchain-version policy.
//!
//! Filesystem hydration is disabled by default. Applications that want rustc
//! replacement spans converted into exact [`Edit`] values must explicitly
//! configure a trusted source root and enable hydration.
//!
//! ## Typed errors
//!
//! Application error enums and structs can implement [`DiagnosticMetadata`]
//! to provide stable diagnostic codes, severity, help, notes, and structured
//! suggestions.
//!
//! This works naturally with `thiserror` because the integration is based on
//! the standard [`std::error::Error`] interface.
//!
//! ## Ecosystem integrations
//!
//! The optional `anyhow` feature preserves Anyhow context/source chains and
//! enriches recognized standard-library failures.
//!
//! The optional `tracing` feature provides a composable tracing-subscriber
//! layer that turns significant runtime events into structured diagnostics.
//!
//! ## Terminal documentation
//!
//! The optional `terminal-docs` feature can fetch documentation and display it
//! directly in a terminal with sanitized remote content and syntax-highlighted
//! examples.
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
//! ## Feature flags
//!
//! - `anyhow` — Anyhow context-chain integration.
//! - `tracing` — structured tracing-event integration.
//! - `compression` — gzip and Zstandard report compression.
//! - `cybercore` — Cybercore theme-schema integration.
//! - `terminal-docs` — terminal documentation retrieval and syntax
//!   highlighting.
//!
//! Typed errors, compiler ingestion, documentation resolution, and `FixPlan`
//! are core features.
//!
//! All optional features are disabled by default.

mod compiler;
mod diagnostic;
mod documentation;
mod fixer;
mod fixplan;
mod intelligence;
mod remediation;
mod reporter;
mod rotation;
mod severity;
mod suggestion;
mod typed;

#[cfg(any(feature = "anyhow", feature = "tracing"))]
pub mod integrations;

#[cfg(feature = "terminal-docs")]
pub mod docs;

pub mod render;

pub use compiler::{CompilerImportError, CompilerImporter};

pub use diagnostic::{Cause, Diagnostic, Label, SourceLocation};

pub use documentation::{DocumentationError, DocumentationResolver};

pub use fixer::{FixCheck, FixError, FixPreview, FixReport, Fixer, RollbackFailure};

pub use fixplan::{
    FileCheck, FileCheckFailure, FixPlan, FixPlanCheck, FixPlanError, FixPlanPreview, FixPlanReport,
};

pub use render::{SeverityTheme, Style, Theme};

pub use reporter::{Compression, Reporter, ReporterBuilder};

pub use rotation::{RotationCadence, RotationPolicy, RotationState};

pub use severity::Severity;

pub use suggestion::{
    Applicability, DocumentationLink, Edit, SuggestedCommand, Suggestion, TextRange,
};

pub use typed::{DiagnosticErrorExt, DiagnosticMetadata};

#[cfg(feature = "anyhow")]
pub use integrations::AnyhowDiagnosticExt;

#[cfg(feature = "tracing")]
pub use integrations::TracingLayer;

#[cfg(feature = "terminal-docs")]
pub use docs::{TerminalDocError, TerminalDocViewer};

pub type Result<T> = std::io::Result<T>;
