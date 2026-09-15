//! Language Server Protocol integration for diagprint.
//!
//! This crate translates revision-aware diagprint diagnostics into LSP
//! diagnostics and converts guarded remediation into versioned workspace-edit
//! code actions.
//!
//! Source revisions and external LSP document versions intentionally remain
//! separate.

mod action;
mod diagnostic;
mod document;
mod error;
mod position;

use diagprint::{CapturedDiagnostic, Diagnostic, SourceSnapshot};
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

    pub fn code_actions(&self, captured: &CapturedDiagnostic) -> Result<Vec<CodeAction>, LspError> {
        action::code_actions(
            captured.diagnostic(),
            captured.sources(),
            &self.documents,
            self.encoding,
        )
    }
}
