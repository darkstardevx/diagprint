use crate::{
    Applicability, Cause, Diagnostic, DiagnosticAttribute, DiagnosticValue, Label, LabelKind,
    REDACTED, Severity, SourceRevision, is_sensitive_key, sanitize_path, sanitize_url,
};
use chrono::{DateTime, Local};
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

/// Policy for free-form text in an external diagnostic export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExportText {
    /// Preserve diagnostic text.
    #[default]
    Preserve,

    /// Replace diagnostic text with the standard redaction marker.
    Redact,
}

impl ExportText {
    fn apply(self, value: &str) -> String {
        match self {
            Self::Preserve => value.to_owned(),
            Self::Redact => REDACTED.to_owned(),
        }
    }
}

/// Policy for source paths in an external diagnostic export.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExportPath {
    /// Omit source paths entirely.
    Omit,

    /// Export only the final filename component.
    #[default]
    FileName,

    /// Export the original source path.
    ///
    /// This can expose usernames, workspace paths, temporary directories, or
    /// other machine-specific information and therefore requires explicit
    /// opt-in.
    FullPath,

    /// Export paths relative to the supplied repository/workspace root.
    ///
    /// Absolute paths outside the root fail closed to their filename. Safe
    /// relative input paths are preserved.
    RepositoryRelative(PathBuf),
}

impl ExportPath {
    fn apply(&self, value: &str) -> Option<String> {
        match self {
            Self::Omit => None,

            Self::FileName => Some(sanitize_path(value)),

            Self::FullPath => Some(value.to_owned()),

            Self::RepositoryRelative(root) => {
                let path = Path::new(value);

                if path.is_relative() {
                    return safe_relative_path(path).or_else(|| Some(sanitize_path(value)));
                }

                path.strip_prefix(root)
                    .ok()
                    .and_then(safe_relative_path)
                    .or_else(|| Some(sanitize_path(value)))
            }
        }
    }
}

fn safe_relative_path(path: &Path) -> Option<String> {
    if path.as_os_str().is_empty() {
        return None;
    }

    let mut parts = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => {
                parts.push(part.to_string_lossy().into_owned());
            }

            Component::CurDir => {}

            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return None;
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// Policy for arbitrary structured diagnostic attributes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExportAttributes {
    /// Export only the structural attribute count.
    #[default]
    Omit,

    /// Export attribute names while redacting every value.
    Redact,

    /// Export non-sensitive attribute values while redacting values whose keys
    /// match diagprint's common sensitive-key rules.
    ///
    /// This is opt-in because arbitrary non-sensitive-looking attributes may
    /// still contain application-specific private information.
    RedactSensitive,

    /// Export every structured attribute value unchanged.
    ///
    /// This is an explicit trust-boundary opt-in.
    Full,
}

/// Policy for remediation information in an external diagnostic export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExportRemediation {
    /// Omit suggestions entirely.
    Omit,

    /// Export suggestion metadata and edit/command counts only.
    ///
    /// Source expectations, replacement text, inserted text, deleted text, and
    /// command contents are never part of the safe export representation.
    #[default]
    MetadataOnly,
}

/// Policy for documentation URLs in an external diagnostic export.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExportUrl {
    /// Omit URLs entirely.
    Omit,

    /// Remove URL user information, query parameters, and fragments.
    #[default]
    Sanitize,

    /// Export the original URL.
    ///
    /// This can expose credentials or sensitive query parameters and therefore
    /// requires explicit opt-in.
    Full,
}

impl ExportUrl {
    fn apply(self, value: &str) -> Option<String> {
        match self {
            Self::Omit => None,
            Self::Sanitize => Some(sanitize_url(value)),
            Self::Full => Some(value.to_owned()),
        }
    }
}

/// Shared privacy policy for external diagnostic serialization.
///
/// The default policy preserves useful diagnostic text while excluding common
/// machine- and application-sensitive data:
///
/// - source paths become filenames;
/// - arbitrary attribute values are omitted;
/// - remediation exports metadata only;
/// - documentation URLs are sanitized;
/// - hostname and process ID are omitted;
/// - application identity remains available;
/// - edit payloads and suggested command contents are never exported.
///
/// [`ExportDiagnostic::from`] uses this default policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPolicy {
    text: ExportText,
    paths: ExportPath,
    attributes: ExportAttributes,
    remediation: ExportRemediation,
    urls: ExportUrl,

    include_application: bool,
    include_hostname: bool,
    include_process_id: bool,
}

impl Default for ExportPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportPolicy {
    /// Creates the conservative default external-export policy.
    pub fn new() -> Self {
        Self {
            text: ExportText::Preserve,
            paths: ExportPath::FileName,
            attributes: ExportAttributes::Omit,
            remediation: ExportRemediation::MetadataOnly,
            urls: ExportUrl::Sanitize,

            include_application: true,
            include_hostname: false,
            include_process_id: false,
        }
    }

    pub fn with_text(mut self, policy: ExportText) -> Self {
        self.text = policy;
        self
    }

    pub fn with_paths(mut self, policy: ExportPath) -> Self {
        self.paths = policy;
        self
    }

    pub fn with_attributes(mut self, policy: ExportAttributes) -> Self {
        self.attributes = policy;
        self
    }

    pub fn with_remediation(mut self, policy: ExportRemediation) -> Self {
        self.remediation = policy;
        self
    }

    pub fn with_urls(mut self, policy: ExportUrl) -> Self {
        self.urls = policy;
        self
    }

    pub fn with_application(mut self, enabled: bool) -> Self {
        self.include_application = enabled;
        self
    }

    pub fn with_hostname(mut self, enabled: bool) -> Self {
        self.include_hostname = enabled;
        self
    }

    pub fn with_process_id(mut self, enabled: bool) -> Self {
        self.include_process_id = enabled;
        self
    }

    pub const fn text(&self) -> ExportText {
        self.text
    }

    pub fn paths(&self) -> &ExportPath {
        &self.paths
    }

    pub const fn attributes(&self) -> ExportAttributes {
        self.attributes
    }

    pub const fn remediation(&self) -> ExportRemediation {
        self.remediation
    }

    pub const fn urls(&self) -> ExportUrl {
        self.urls
    }

    pub const fn includes_application(&self) -> bool {
        self.include_application
    }

    pub const fn includes_hostname(&self) -> bool {
        self.include_hostname
    }

    pub const fn includes_process_id(&self) -> bool {
        self.include_process_id
    }

    pub(crate) fn apply_text(&self, value: &str) -> String {
        self.text.apply(value)
    }

    pub(crate) fn apply_path(&self, value: &str) -> Option<String> {
        self.paths.apply(value)
    }
}

/// External, privacy-conscious representation of a diagnostic.
///
/// This representation is deliberately separate from [`struct@Diagnostic`].
/// Internal diagnostics retain complete remediation and process context while
/// exporters choose an explicit [`ExportPolicy`].
#[derive(Debug, Clone, Serialize)]
pub struct ExportDiagnostic {
    pub report_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Local>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub application: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    pub severity: Severity,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    pub message: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<DiagnosticAttribute>,

    /// Number of structured attributes present on the internal diagnostic.
    pub attribute_count: usize,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<ExportLabel>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Cause>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<ExportSuggestion>,
}

impl ExportDiagnostic {
    /// Builds an external representation using the supplied policy.
    pub fn with_policy(diagnostic: &Diagnostic, policy: &ExportPolicy) -> Self {
        Self {
            report_id: diagnostic.report_id,

            session_id: diagnostic.session_id,

            timestamp: diagnostic.timestamp,

            application: policy
                .include_application
                .then(|| diagnostic.application.clone()),

            pid: policy.include_process_id.then_some(diagnostic.pid),

            hostname: policy.include_hostname.then(|| diagnostic.hostname.clone()),

            severity: diagnostic.severity,

            code: diagnostic.code.clone(),

            message: policy.text.apply(&diagnostic.message),

            attributes: export_attributes(&diagnostic.attributes, policy.attributes),

            attribute_count: diagnostic.attributes.len(),

            labels: diagnostic
                .labels
                .iter()
                .map(|label| ExportLabel::with_policy(label, policy))
                .collect(),

            notes: diagnostic
                .notes
                .iter()
                .map(|note| policy.text.apply(note))
                .collect(),

            help: diagnostic
                .help
                .as_deref()
                .map(|help| policy.text.apply(help)),

            cause: diagnostic
                .cause
                .as_ref()
                .map(|cause| export_cause(cause, policy.text)),

            suggestions: match policy.remediation {
                ExportRemediation::Omit => Vec::new(),

                ExportRemediation::MetadataOnly => diagnostic
                    .suggestions
                    .iter()
                    .map(|suggestion| ExportSuggestion::with_policy(suggestion, policy))
                    .collect(),
            },
        }
    }
}

impl From<&Diagnostic> for ExportDiagnostic {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self::with_policy(diagnostic, &ExportPolicy::default())
    }
}

fn export_attributes(
    attributes: &[DiagnosticAttribute],
    policy: ExportAttributes,
) -> Vec<DiagnosticAttribute> {
    match policy {
        ExportAttributes::Omit => Vec::new(),

        ExportAttributes::Redact => attributes
            .iter()
            .map(|attribute| {
                DiagnosticAttribute::new(attribute.name.clone(), DiagnosticValue::from(REDACTED))
            })
            .collect(),

        ExportAttributes::RedactSensitive => attributes
            .iter()
            .map(|attribute| {
                if is_sensitive_key(&attribute.name) {
                    DiagnosticAttribute::new(
                        attribute.name.clone(),
                        DiagnosticValue::from(REDACTED),
                    )
                } else {
                    attribute.clone()
                }
            })
            .collect(),

        ExportAttributes::Full => attributes.to_vec(),
    }
}

fn export_cause(cause: &Cause, policy: ExportText) -> Cause {
    Cause {
        message: policy.apply(&cause.message),

        source: cause
            .source
            .as_deref()
            .map(|source| Box::new(export_cause(source, policy))),
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

impl ExportLabel {
    fn with_policy(label: &Label, policy: &ExportPolicy) -> Self {
        Self {
            kind: label.kind,

            location: ExportSourceLocation {
                file: policy.paths.apply(&label.location.file),

                line: label.location.line,

                column: label.location.column,

                revision: label.location.revision,
            },

            length: label.length,

            message: label
                .message
                .as_deref()
                .map(|message| policy.text.apply(message)),
        }
    }
}

impl From<&Label> for ExportLabel {
    fn from(label: &Label) -> Self {
        Self::with_policy(label, &ExportPolicy::default())
    }
}

/// Privacy-conscious source location.
#[derive(Debug, Clone, Serialize)]
pub struct ExportSourceLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,

    pub line: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<SourceRevision>,
}

/// Metadata-only representation of a remediation suggestion.
///
/// Edit and command payloads are intentionally not represented by this type.
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

impl ExportSuggestion {
    fn with_policy(suggestion: &crate::Suggestion, policy: &ExportPolicy) -> Self {
        Self {
            title: policy.text.apply(&suggestion.title),

            explanation: suggestion
                .explanation
                .as_deref()
                .map(|explanation| policy.text.apply(explanation)),

            applicability: suggestion.applicability,

            documentation: suggestion
                .documentation
                .iter()
                .map(|link| ExportDocumentationLink {
                    label: policy.text.apply(&link.label),

                    url: policy.urls.apply(&link.url),

                    language_hint: link.language_hint.clone(),
                })
                .collect(),

            edit_count: suggestion.edits.len(),

            command_count: suggestion.commands.len(),
        }
    }
}

impl From<&crate::Suggestion> for ExportSuggestion {
    fn from(suggestion: &crate::Suggestion) -> Self {
        Self::with_policy(suggestion, &ExportPolicy::default())
    }
}

/// Sanitized documentation link.
#[derive(Debug, Clone, Serialize)]
pub struct ExportDocumentationLink {
    pub label: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_hint: Option<String>,
}
