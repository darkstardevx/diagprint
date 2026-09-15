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
//! - [`CapturedDiagnostic`] pairs a diagnostic with its immutable source snapshot.
//! - [`Suggestion`] describes a possible resolution.
//! - [`Fixer`] validates and applies guarded structured edits.
//! - [`FixPlan`] coordinates transactional multi-file remediation.
//! - [`InteropDiagnostic`] is the dependency-free interoperability boundary.
//! - [`SourceCache`] supplies virtual and cached source text to renderers.
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
//! ## Virtual and cached source text
//!
//! [`SourceCache`] stores named source text independently from diagnostics.
//! [`SourceProvider`] lets parsers, bridges, and other integrations populate
//! that cache without coupling the renderer to a specific ecosystem.
//! [`SourceSnapshot`] captures an immutable point-in-time source view so old
//! diagnostics can render against the exact text they were created from.
//! [`SourceRevision`] identifies successive versions of one named source and
//! allows snapshots to detect whether a diagnostic-time buffer is still current.
//! Terminal rendering can prefer cached source and fall back to filesystem
//! reads when no cached source exists.
//!
//! This supports editor buffers, generated files, parser inputs, compiler
//! virtual files, and other source text which may never exist on disk.
//!
//! Source contents are not serialized into [`Diagnostic`] JSON output.
//!
//! ## Generic interoperability
//!
//! [`InteropDiagnostic`] is a dependency-free, owned diagnostic protocol for
//! compilers, linters, parsers, language tools, and third-party adapters.
//!
//! It preserves:
//!
//! - severity;
//! - diagnostic code;
//! - message and help;
//! - notes;
//! - primary and secondary source labels;
//! - cause chains;
//! - documentation links;
//! - related diagnostics.
//!
//! Custom producers can implement [`InteropDiagnosticSource`] and gain direct
//! conversion through [`InteropDiagnosticSourceExt`].
//!
//! Automatic edits and executable commands are deliberately excluded from the
//! generic protocol. Remediation requires stronger ecosystem-specific safety
//! guarantees.
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
//! The `miette` feature consumes miette's structured diagnostic protocol,
//! including source labels, causes, and related diagnostics.
//!
//! The `codespan-reporting` feature resolves codespan's file IDs and byte
//! ranges into [`InteropDiagnostic`] before conversion to diagprint.
//!
//! The `ariadne` feature provides a structured bridge which can emit both an
//! Ariadne report and a diagprint [`InteropDiagnostic`] from the same source
//! metadata without parsing rendered terminal output.
//!
//! The `annotate-snippets` feature provides the same dual-output model for
//! annotate-snippets reports while validating its byte-oriented source spans.
//!
//! The `anyhow` feature preserves Anyhow context/source chains.
//!
//! The `tracing` feature turns significant tracing events into structured
//! diagnostics.
//!
//! External adapters never need to parse another library's pretty terminal
//! rendering.
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
//! - `ariadne` — structured Ariadne/diagprint bridge.
//! - `annotate-snippets` — structured annotate-snippets/diagprint bridge.
//! - `miette` — miette diagnostic-protocol integration.
//! - `codespan-reporting` — codespan-reporting diagnostic integration.
//! - `tracing` — structured tracing-event integration.
//! - `compression` — gzip and Zstandard report compression.
//! - `cybercore` — Cybercore theme-schema integration.
//! - `terminal-docs` — terminal documentation retrieval and syntax
//!   highlighting.
//!
//! Generic interoperability, typed errors, compiler/Cargo ingestion,
//! documentation resolution, and `FixPlan` are core features.
//!
//! All optional features are disabled by default.

mod captured;
mod cargo;
mod compiler;
mod diagnostic;
mod documentation;
mod fixer;
mod fixplan;
mod intelligence;
pub mod interop;
mod remediation;
mod reporter;
mod rotation;
mod severity;
mod source;
mod suggestion;
mod typed;

#[cfg(any(
    feature = "anyhow",
    feature = "ariadne",
    feature = "annotate-snippets",
    feature = "codespan-reporting",
    feature = "miette",
    feature = "tracing"
))]
pub mod integrations;

#[cfg(feature = "terminal-docs")]
pub mod docs;

pub mod render;

pub use captured::CapturedDiagnostic;

pub use cargo::{
    CargoArtifact, CargoBuildFinished, CargoBuildScript, CargoBuildSummary,
    CargoCompilerDiagnostic, CargoDependency, CargoDependencyKind, CargoImportError, CargoMessage,
    CargoPackage, CargoPackageIdentity, CargoProfile, CargoStreamImporter, CargoTarget,
    CargoUnknownMessage, CargoWorkspace,
};

pub use compiler::{CompilerImportError, CompilerImporter};

pub use diagnostic::{Cause, Diagnostic, Label, LabelKind, SourceLocation};

pub use documentation::{DocumentationError, DocumentationResolver};

pub use fixer::{FixCheck, FixError, FixPreview, FixReport, Fixer, RollbackFailure};

pub use fixplan::{
    FileCheck, FileCheckFailure, FixPlan, FixPlanCheck, FixPlanError, FixPlanPreview, FixPlanReport,
};

pub use interop::{
    DiagnosticTree, InteropDiagnostic, InteropDiagnosticSource, InteropDiagnosticSourceExt,
    InteropLabel,
};

pub use render::{SeverityTheme, Style, Theme};

pub use reporter::{Compression, Reporter, ReporterBuilder};

pub use rotation::{RotationCadence, RotationPolicy, RotationState};

pub use severity::Severity;

pub use source::{SourceCache, SourceEntry, SourceProvider, SourceRevision, SourceSnapshot};

pub use suggestion::{
    Applicability, DocumentationLink, Edit, SuggestedCommand, Suggestion, TextRange,
};

pub use typed::{DiagnosticErrorExt, DiagnosticMetadata};

#[cfg(feature = "ariadne")]
pub use integrations::{
    AriadneBridge, AriadneBridgeError, AriadneBridgeLabel, AriadneOwnedSpan, AriadneSpan,
};

#[cfg(feature = "annotate-snippets")]
pub use integrations::{
    AnnotateSnippetsBridge, AnnotateSnippetsBridgeError, AnnotateSnippetsLabel,
};

#[cfg(feature = "anyhow")]
pub use integrations::AnyhowDiagnosticExt;

#[cfg(feature = "codespan-reporting")]
pub use integrations::CodespanDiagnosticExt;

#[cfg(feature = "miette")]
pub use integrations::{MietteDiagnosticExt, MietteDiagnosticTree, MietteReportExt};

#[cfg(feature = "tracing")]
pub use integrations::TracingLayer;

#[cfg(feature = "terminal-docs")]
pub use docs::{TerminalDocError, TerminalDocViewer};

pub type Result<T> = std::io::Result<T>;
