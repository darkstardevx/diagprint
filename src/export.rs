use crate::{
    Applicability, Cause, Diagnostic, Label, LabelKind, Severity, SourceRevision, sanitize_path,
    sanitize_url,
};
use chrono::{DateTime, Local};
use serde::Serialize;
use uuid::Uuid;

/// External, privacy-conscious representation of a diagnostic.
///
/// This view deliberately excludes:
///
/// - process IDs and hostnames;
/// - arbitrary diagnostic attribute values;
/// - remediation source text;
/// - replacement/inserted/deleted text;
/// - suggested command contents.
///
/// It is intended for external serialization and transport. The original
/// [`struct@Diagnostic`] remains the authoritative internal representation.
#[derive(Debug, Clone, Serialize)]
pub struct ExportDiagnostic {
    pub report_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Local>,
    pub application: String,

    pub severity: Severity,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    pub message: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<ExportLabel>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Cause>,

    /// Number of structured attributes present on the internal diagnostic.
    ///
    /// Attribute values are omitted from the default external representation.
    pub attribute_count: usize,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<ExportSuggestion>,
}

impl From<&Diagnostic> for ExportDiagnostic {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            report_id: diagnostic.report_id,

            session_id: diagnostic.session_id,

            timestamp: diagnostic.timestamp,

            application: diagnostic.application.clone(),

            severity: diagnostic.severity,

            code: diagnostic.code.clone(),

            message: diagnostic.message.clone(),

            labels: diagnostic.labels.iter().map(ExportLabel::from).collect(),

            notes: diagnostic.notes.clone(),

            help: diagnostic.help.clone(),

            cause: diagnostic.cause.clone(),

            attribute_count: diagnostic.attributes.len(),

            suggestions: diagnostic
                .suggestions
                .iter()
                .map(ExportSuggestion::from)
                .collect(),
        }
    }
}

/// Privacy-conscious source label.
#[derive(Debug, Clone, Serialize)]
pub struct ExportLabel {
    #[serde(skip_serializing_if = "label_kind_is_primary")]
    pub kind: LabelKind,
    pub location: ExportSourceLocation,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn label_kind_is_primary(kind: &LabelKind) -> bool {
    *kind == LabelKind::Primary
}

impl From<&Label> for ExportLabel {
    fn from(label: &Label) -> Self {
        Self {
            kind: label.kind,

            location: ExportSourceLocation {
                file: sanitize_path(&label.location.file),

                line: label.location.line,

                column: label.location.column,

                revision: label.location.revision,
            },

            length: label.length,

            message: label.message.clone(),
        }
    }
}

/// Privacy-conscious source location.
#[derive(Debug, Clone, Serialize)]
pub struct ExportSourceLocation {
    pub file: String,
    pub line: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<SourceRevision>,
}

/// Metadata-only representation of a remediation suggestion.
///
/// Edit and command payloads are intentionally not exported.
#[derive(Debug, Clone, Serialize)]
pub struct ExportSuggestion {
    pub title: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,

    pub applicability: Applicability,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub documentation: Vec<ExportDocumentationLink>,

    pub edit_count: usize,
    pub command_count: usize,
}

impl From<&crate::Suggestion> for ExportSuggestion {
    fn from(suggestion: &crate::Suggestion) -> Self {
        Self {
            title: suggestion.title.clone(),

            explanation: suggestion.explanation.clone(),

            applicability: suggestion.applicability,

            documentation: suggestion
                .documentation
                .iter()
                .map(|link| ExportDocumentationLink {
                    label: link.label.clone(),

                    url: sanitize_url(&link.url),

                    language_hint: link.language_hint.clone(),
                })
                .collect(),

            edit_count: suggestion.edits.len(),

            command_count: suggestion.commands.len(),
        }
    }
}

/// Sanitized documentation link.
#[derive(Debug, Clone, Serialize)]
pub struct ExportDocumentationLink {
    pub label: String,
    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_hint: Option<String>,
}
