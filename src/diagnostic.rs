use crate::{
    DiagnosticAttribute, DiagnosticValue, Severity, SourceCache, SourceRevision, SourceSnapshot,
    Suggestion,
};
use chrono::{DateTime, Local};
use serde::Serialize;
use std::error::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelKind {
    #[default]
    Primary,
    Secondary,
}

impl LabelKind {
    fn is_primary(&self) -> bool {
        *self == Self::Primary
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,

    /// Source revision this location was produced against.
    ///
    /// Unrevisioned diagnostics leave this unset, preserving the historical
    /// diagprint data model and JSON representation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<SourceRevision>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Label {
    #[serde(skip_serializing_if = "LabelKind::is_primary")]
    pub kind: LabelKind,

    pub location: SourceLocation,
    pub length: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cause {
    pub message: String,
    pub source: Option<Box<Cause>>,
}

impl Cause {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    pub fn caused_by(mut self, cause: Cause) -> Self {
        self.source = Some(Box::new(cause));
        self
    }

    pub fn from_error(error: &(dyn Error + 'static)) -> Self {
        let mut root = Cause::new(error.to_string());
        let mut tail = &mut root;
        let mut next = error.source();

        while let Some(error) = next {
            tail = tail
                .source
                .insert(Box::new(Cause::new(error.to_string())))
                .as_mut();

            next = error.source();
        }

        root
    }

    pub fn iter(&self) -> CauseIter<'_> {
        CauseIter { next: Some(self) }
    }
}

pub struct CauseIter<'a> {
    next: Option<&'a Cause>,
}

impl<'a> Iterator for CauseIter<'a> {
    type Item = &'a Cause;

    fn next(&mut self) -> Option<Self::Item> {
        let cause = self.next?;

        self.next = cause.source.as_deref();

        Some(cause)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub report_id: Uuid,
    pub session_id: Uuid,
    pub timestamp: DateTime<Local>,
    pub application: String,
    pub pid: u32,
    pub hostname: String,

    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<DiagnosticAttribute>,

    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Option<String>,
    pub cause: Option<Cause>,

    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    pub(crate) fn new(
        session_id: Uuid,
        application: String,
        severity: Severity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            report_id: Uuid::now_v7(),
            session_id,
            timestamp: Local::now(),
            application,
            pid: std::process::id(),

            hostname: hostname::get()
                .ok()
                .and_then(|hostname| hostname.into_string().ok())
                .unwrap_or_else(|| "unknown".into()),

            severity,
            code: None,
            message: message.into(),

            attributes: Vec::new(),

            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
            cause: None,

            suggestions: Vec::new(),
        }
    }

    pub fn code(mut self, value: impl Into<String>) -> Self {
        self.code = Some(value.into());
        self
    }

    /// Adds one structured diagnostic attribute.
    pub fn attribute(mut self, name: impl Into<String>, value: impl Into<DiagnosticValue>) -> Self {
        self.attributes.push(DiagnosticAttribute::new(name, value));

        self
    }

    /// Adds multiple structured diagnostic attributes.
    pub fn attributes(mut self, attributes: impl IntoIterator<Item = DiagnosticAttribute>) -> Self {
        self.attributes.extend(attributes);
        self
    }

    pub fn note(mut self, value: impl Into<String>) -> Self {
        self.notes.push(value.into());
        self
    }

    pub fn help(mut self, value: impl Into<String>) -> Self {
        self.help = Some(value.into());
        self
    }

    pub fn cause(mut self, value: impl Into<String>) -> Self {
        self.cause = Some(match self.cause.take() {
            None => Cause::new(value),
            Some(old) => old.caused_by(Cause::new(value)),
        });

        self
    }

    pub fn cause_chain(mut self, cause: Cause) -> Self {
        self.cause = Some(cause);
        self
    }

    pub fn from_error(mut self, error: &(dyn Error + 'static)) -> Self {
        self.cause = Some(Cause::from_error(error));
        self
    }

    pub fn label(
        self,
        file: impl Into<String>,
        line: u32,
        column: Option<u32>,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.label_with_location(
            LabelKind::Primary,
            SourceLocation {
                file: file.into(),
                line,
                column,
                revision: None,
            },
            length,
            message,
        )
    }

    pub fn label_at_revision(
        self,
        file: impl Into<String>,
        revision: SourceRevision,
        line: u32,
        column: Option<u32>,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.label_with_location(
            LabelKind::Primary,
            SourceLocation {
                file: file.into(),
                line,
                column,
                revision: Some(revision),
            },
            length,
            message,
        )
    }

    pub fn secondary_label(
        self,
        file: impl Into<String>,
        line: u32,
        column: Option<u32>,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.label_with_location(
            LabelKind::Secondary,
            SourceLocation {
                file: file.into(),
                line,
                column,
                revision: None,
            },
            length,
            message,
        )
    }

    pub fn secondary_label_at_revision(
        self,
        file: impl Into<String>,
        revision: SourceRevision,
        line: u32,
        column: Option<u32>,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.label_with_location(
            LabelKind::Secondary,
            SourceLocation {
                file: file.into(),
                line,
                column,
                revision: Some(revision),
            },
            length,
            message,
        )
    }

    pub fn label_with_kind(
        self,
        kind: LabelKind,
        file: impl Into<String>,
        line: u32,
        column: Option<u32>,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.label_with_location(
            kind,
            SourceLocation {
                file: file.into(),
                line,
                column,
                revision: None,
            },
            length,
            message,
        )
    }

    /// Adds a label using a fully specified source location.
    ///
    /// This is the general form for callers that already have structured
    /// location metadata, including an optional source revision.
    pub fn label_with_location(
        mut self,
        kind: LabelKind,
        location: SourceLocation,
        length: Option<usize>,
        message: Option<impl Into<String>>,
    ) -> Self {
        self.labels.push(Label {
            kind,
            location,
            length,
            message: message.map(Into::into),
        });

        self
    }

    pub fn source(self, file: impl Into<String>, line: u32, column: Option<u32>) -> Self {
        self.label(file, line, column, None, None::<String>)
    }

    pub fn source_at_revision(
        self,
        file: impl Into<String>,
        revision: SourceRevision,
        line: u32,
        column: Option<u32>,
    ) -> Self {
        self.label_at_revision(file, revision, line, column, None, None::<String>)
    }

    pub fn bind_source_revisions(mut self, sources: &SourceSnapshot) -> Self {
        for label in &mut self.labels {
            if label.location.revision.is_some() {
                continue;
            }

            if let Some(revision) = sources.revision(&label.location.file) {
                label.location.revision = Some(revision);
            }
        }

        self
    }

    /// Binds unversioned labels to the revisions currently stored in `sources`.
    ///
    /// Prefer [`Self::bind_source_revisions`] when a diagnostic was produced
    /// from a snapshot, because a live cache may change concurrently.
    pub fn bind_current_source_revisions(mut self, sources: &SourceCache) -> Self {
        for label in &mut self.labels {
            if label.location.revision.is_some() {
                continue;
            }

            if let Some(revision) = sources.revision(&label.location.file) {
                label.location.revision = Some(revision);
            }
        }

        self
    }

    pub fn has_revisioned_sources(&self) -> bool {
        self.labels
            .iter()
            .any(|label| label.location.revision.is_some())
    }

    /// Returns true when any revision-bound label no longer matches the live
    /// source cache.
    ///
    /// Unversioned labels are ignored.
    pub fn has_stale_sources(&self, sources: &SourceCache) -> bool {
        self.labels.iter().any(|label| {
            let Some(expected) = label.location.revision else {
                return false;
            };

            sources.revision(&label.location.file) != Some(expected)
        })
    }

    pub fn labels(mut self, labels: impl IntoIterator<Item = Label>) -> Self {
        self.labels.extend(labels);
        self
    }

    pub fn suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub fn suggestions(mut self, suggestions: impl IntoIterator<Item = Suggestion>) -> Self {
        self.suggestions.extend(suggestions);
        self
    }
}
