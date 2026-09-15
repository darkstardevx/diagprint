//! # diagprint
//!
//! `diagprint` provides structured diagnostics, rich terminal rendering,
//! persistent reports, guarded remediation, compiler/Cargo intelligence,
//! version-aware documentation, and Rust ecosystem interoperability.
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
//! - [`CompilerImporter`] consumes structured rustc diagnostics.
//! - [`CargoWorkspace`] models Cargo package/dependency metadata.
//! - [`CargoStreamImporter`] consumes Cargo build-message streams.
//!
//! Calling a renderer never modifies source files.
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
//! ## Cargo intelligence
//!
//! [`CargoWorkspace`] consumes `cargo metadata --format-version=1` output and
//! preserves package identity, versions, workspace membership, targets,
//! features, and resolved dependency context.
//!
//! [`CargoStreamImporter`] consumes Cargo `--message-format=json` output and
//! recognizes compiler diagnostics, compiler artifacts, build-script results,
//! and build completion.
//!
//! Unknown future Cargo message kinds are retained as [`CargoMessage::Unknown`]
//! rather than rejected.
//!
//! ## Documentation intelligence
//!
//! [`DocumentationResolver`] can build package/version catalogs from
//! `Cargo.lock` or [`CargoWorkspace`].
//!
//! Ambiguous package versions remain ambiguous rather than being guessed.
//!
//! ## Ecosystem interoperability
//!
//! Optional adapters allow existing Rust diagnostic ecosystems to feed
//! structured data into `diagprint`.
//!
//! The `miette` feature consumes [`miette::Diagnostic`] metadata directly:
//!
//! - message;
//! - severity;
//! - code;
//! - help;
//! - documentation URL;
//! - source labels;
//! - diagnostic/source chains;
//! - related diagnostics.
//!
//! Miette's `related()` relationship is preserved by
//! [`MietteDiagnosticTree`] rather than flattened into strings.
//!
//! The adapter does not parse miette's rendered report output and does not
//! manufacture machine-applicable fixes.
//!
//! The `anyhow` feature preserves Anyhow context/source chains and enriches
//! recognized standard-library failures.
//!
//! The `tracing` feature provides a composable tracing-subscriber layer that
//! turns significant runtime events into structured diagnostics.
//!
//! ## Compiler diagnostics
//!
//! [`CompilerImporter`] consumes structured rustc JSON diagnostics directly.
//! It preserves compiler severity, Rust error codes, source spans, notes,
//! help, structured replacements, and applicability.
//!
//! Filesystem hydration is disabled by default.
//!
//! ## Typed errors
//!
//! Application error enums and structs can implement [`DiagnosticMetadata`]
//! to provide stable diagnostic codes, severity, help, notes, and structured
//! suggestions.
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
//!
//! The optional `cybercore` feature maps the Cybercore semantic palette into
//! `diagprint` rather than copying Cybercore color values.
//!
//! ## Feature flags
//!
//! - `anyhow` — Anyhow context-chain integration.
//! - `miette` — miette diagnostic-protocol integration.
//! - `tracing` — structured tracing-event integration.
//! - `compression` — gzip and Zstandard report compression.
//! - `cybercore` — Cybercore theme-schema integration.
//! - `terminal-docs` — terminal documentation retrieval and syntax
//!   highlighting.
//!
//! Typed errors, compiler/Cargo ingestion, documentation resolution, and
//! `FixPlan` are core features.
//!
//! All optional features are disabled by default.

mod cargo;
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

#[cfg(any(feature = "anyhow", feature = "miette", feature = "tracing"))]
pub mod integrations;

#[cfg(feature = "terminal-docs")]
pub mod docs;

pub mod render;

pub use cargo::{
    CargoArtifact, CargoBuildFinished, CargoBuildScript, CargoBuildSummary,
    CargoCompilerDiagnostic, CargoDependency, CargoDependencyKind, CargoImportError, CargoMessage,
    CargoPackage, CargoPackageIdentity, CargoProfile, CargoStreamImporter, CargoTarget,
    CargoUnknownMessage, CargoWorkspace,
};

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

#[cfg(feature = "miette")]
pub use integrations::{MietteDiagnosticExt, MietteDiagnosticTree, MietteReportExt};

#[cfg(feature = "tracing")]
pub use integrations::TracingLayer;

#[cfg(feature = "terminal-docs")]
pub use docs::{TerminalDocError, TerminalDocViewer};

pub type Result<T> = std::io::Result<T>;
