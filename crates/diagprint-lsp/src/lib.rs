//! Language Server Protocol integration for diagprint.
//!
//! This crate translates revision-aware diagprint diagnostics into LSP
//! diagnostics and converts guarded remediation into versioned workspace-edit
//! code actions.
//!
//! Source revisions and external LSP document versions intentionally remain
//! separate.

mod action;
mod batch;
mod diagnostic;
mod document;
mod error;
mod position;

use diagprint::{CapturedDiagnostic, Diagnostic, DiagnosticReport, SourceSnapshot};
use lsp_types::{CodeAction, Diagnostic as LspDiagnostic, PublishDiagnosticsParams};

pub use document::{DocumentMap, LspDocument};

pub use error::LspError;
pub use position::PositionEncoding;

pub use lsp_types;

#[derive(Debug, Clone)]
pub struct LspAdapter {
    documents: DocumentMap,
    encoding: PositionEncoding,
}

impl LspAdapter {
    pub fn new(documents: DocumentMap, encoding: PositionEncoding) -> Self {
        Self {
            documents,
            encoding,
        }
    }

    pub fn utf16(documents: DocumentMap) -> Self {
        Self::new(documents, PositionEncoding::Utf16)
    }

    pub fn documents(&self) -> &DocumentMap {
        &self.documents
    }

    pub fn documents_mut(&mut self) -> &mut DocumentMap {
        &mut self.documents
    }

    pub const fn encoding(&self) -> PositionEncoding {
        self.encoding
    }

    pub fn diagnostic(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
    ) -> Result<LspDiagnostic, LspError> {
        diagnostic::to_lsp_diagnostic(diagnostic, sources, &self.documents, self.encoding)
    }

    pub fn captured_diagnostic(
        &self,
        captured: &CapturedDiagnostic,
    ) -> Result<LspDiagnostic, LspError> {
        self.diagnostic(captured.diagnostic(), captured.sources())
    }

    pub fn publish_captured(
        &self,
        captured: &CapturedDiagnostic,
    ) -> Result<PublishDiagnosticsParams, LspError> {
        diagnostic::publish_diagnostic(
            captured.diagnostic(),
            captured.sources(),
            &self.documents,
            self.encoding,
        )
    }

    /// Converts a report into one publishDiagnostics payload per primary
    /// document.
    ///
    /// Diagnostics targeting the same primary source are grouped together.
    /// Output order is deterministic by diagprint source name.
    pub fn publish_report(
        &self,
        report: &DiagnosticReport,
        sources: &SourceSnapshot,
    ) -> Result<Vec<PublishDiagnosticsParams>, LspError> {
        batch::publish_report(report, sources, &self.documents, self.encoding)
    }

    /// Builds CodeActions for every diagnostic in a report against one
    /// immutable source snapshot.
    pub fn code_actions_report(
        &self,
        report: &DiagnosticReport,
        sources: &SourceSnapshot,
    ) -> Result<Vec<CodeAction>, LspError> {
        batch::code_actions_report(report, sources, &self.documents, self.encoding)
    }

    pub fn code_actions(&self, captured: &CapturedDiagnostic) -> Result<Vec<CodeAction>, LspError> {
        action::code_actions(
            captured.diagnostic(),
            captured.sources(),
            &self.documents,
            self.encoding,
        )
    }
}
