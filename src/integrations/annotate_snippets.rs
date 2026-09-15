//! Structured bridge between `annotate-snippets` and diagprint.
//!
//! `annotate-snippets` is primarily a diagnostic construction/rendering API.
//! Finished groups, snippets, annotations, and messages intentionally keep
//! their structured fields private.
//!
//! Consequently this adapter does not parse rendered output.
//!
//! [`AnnotateSnippetsBridge`] owns the diagnostic structure before rendering
//! and can produce both:
//!
//! - an owned set of `annotate-snippets` groups; and
//! - diagprint's dependency-neutral [`crate::InteropDiagnostic`].
//!
//! `annotate-snippets` annotation spans are byte ranges. The bridge validates
//! those ranges and converts them into one-based line/column locations and
//! character lengths for diagprint.
//!
//! No remediation edits or executable commands are inferred.

use crate::{
    Diagnostic, InteropDiagnostic, InteropDiagnosticSource, InteropLabel, LabelKind, Reporter,
    Severity, SourceCache, SourceProvider,
};
use ::annotate_snippets::{AnnotationKind, Group, Level, Snippet};
use std::{collections::BTreeMap, error::Error, fmt, ops::Range};

/// Structured source annotation shared by both renderers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotateSnippetsLabel {
    pub kind: LabelKind,
    pub file: String,
    pub range: Range<usize>,
    pub message: Option<String>,
}

impl AnnotateSnippetsLabel {
    pub fn primary(file: impl Into<String>, range: Range<usize>) -> Self {
        Self::new(LabelKind::Primary, file, range)
    }

    pub fn secondary(file: impl Into<String>, range: Range<usize>) -> Self {
        Self::new(LabelKind::Secondary, file, range)
    }

    pub fn new(kind: LabelKind, file: impl Into<String>, range: Range<usize>) -> Self {
        Self {
            kind,
            file: file.into(),
            range,
            message: None,
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

/// Failure while resolving an annotate-snippets byte span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotateSnippetsBridgeError {
    MissingSource {
        file: String,
    },

    InvalidSpan {
        file: String,
        start: usize,
        end: usize,
        source_len: usize,
    },

    InvalidUtf8Boundary {
        file: String,
        start: usize,
        end: usize,
    },
}

impl fmt::Display for AnnotateSnippetsBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource { file } => {
                write!(f, "annotate-snippets source {file:?} was not registered")
            }

            Self::InvalidSpan {
                file,
                start,
                end,
                source_len,
            } => write!(
                f,
                "annotate-snippets byte range {start}..{end} for {file:?} \
                 is invalid for a source containing {source_len} bytes"
            ),

            Self::InvalidUtf8Boundary { file, start, end } => write!(
                f,
                "annotate-snippets byte range {start}..{end} for {file:?} \
                 does not align to UTF-8 character boundaries"
            ),
        }
    }
}

impl Error for AnnotateSnippetsBridgeError {}

/// Structured diagnostic which can feed both annotate-snippets and diagprint.
#[derive(Debug, Clone)]
pub struct AnnotateSnippetsBridge {
    severity: Severity,
    code: Option<String>,
    message: String,
    help: Option<String>,
    notes: Vec<String>,

    labels: Vec<AnnotateSnippetsLabel>,
    sources: BTreeMap<String, String>,
}

impl AnnotateSnippetsBridge {
    pub fn new(severity: Severity, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: None,
            message: message.into(),
            help: None,
            notes: Vec::new(),
            labels: Vec::new(),
            sources: BTreeMap::new(),
        }
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn source(mut self, file: impl Into<String>, source: impl Into<String>) -> Self {
        self.sources.insert(file.into(), source.into());
        self
    }

    pub fn label(mut self, label: AnnotateSnippetsLabel) -> Self {
        self.labels.push(label);
        self
    }

    pub fn primary_label(
        self,
        file: impl Into<String>,
        range: Range<usize>,
        message: impl Into<String>,
    ) -> Self {
        self.label(AnnotateSnippetsLabel::primary(file, range).message(message))
    }

    pub fn secondary_label(
        self,
        file: impl Into<String>,
        range: Range<usize>,
        message: impl Into<String>,
    ) -> Self {
        self.label(AnnotateSnippetsLabel::secondary(file, range).message(message))
    }

    /// Validates all byte spans.
    pub fn validate(&self) -> Result<(), AnnotateSnippetsBridgeError> {
        for label in &self.labels {
            self.resolve_label(label)?;
        }

        Ok(())
    }

    /// Builds owned annotate-snippets groups.
    ///
    /// `annotate_snippets::Report` is a borrowed slice of groups, so callers
    /// can pass `&groups` directly to `Renderer::render`.
    pub fn to_annotate_snippets_groups(
        &self,
    ) -> Result<Vec<Group<'static>>, AnnotateSnippetsBridgeError> {
        self.validate()?;

        let level = level_from_severity(self.severity);

        let title = if let Some(code) = &self.code {
            level.primary_title(self.message.clone()).id(code.clone())
        } else {
            level.primary_title(self.message.clone())
        };

        let mut group = Group::with_title(title);

        for (file, source) in &self.sources {
            let relevant: Vec<_> = self
                .labels
                .iter()
                .filter(|label| label.file == *file)
                .collect();

            if relevant.is_empty() {
                continue;
            }

            let mut snippet = Snippet::source(source.clone()).path(file.clone());

            for label in relevant {
                let kind = match label.kind {
                    LabelKind::Primary => AnnotationKind::Primary,
                    LabelKind::Secondary => AnnotationKind::Context,
                };

                let annotation = kind.span(label.range.clone());

                let annotation = if let Some(message) = &label.message {
                    annotation.label(message.clone())
                } else {
                    annotation
                };

                snippet = snippet.annotation(annotation);
            }

            group = group.element(snippet);
        }

        for note in &self.notes {
            group = group.element(Level::NOTE.message(note.clone()));
        }

        if let Some(help) = &self.help {
            group = group.element(Level::HELP.message(help.clone()));
        }

        Ok(vec![group])
    }

    /// Converts into diagprint's dependency-neutral protocol.
    ///
    /// Unresolvable labels become notes rather than guessed locations.
    pub fn to_interop_diagnostic(&self) -> InteropDiagnostic {
        let mut diagnostic = InteropDiagnostic::new(self.message.clone()).severity(self.severity);

        if let Some(code) = &self.code {
            diagnostic = diagnostic.code(code.clone());
        }

        if let Some(help) = &self.help {
            diagnostic = diagnostic.help(help.clone());
        }

        for note in &self.notes {
            diagnostic = diagnostic.note(note.clone());
        }

        for label in &self.labels {
            match self.resolve_label(label) {
                Ok(resolved) => {
                    let mut interop =
                        InteropLabel::new(label.kind, label.file.clone(), resolved.line)
                            .column(resolved.column);

                    if let Some(length) = resolved.length {
                        interop = interop.length(length);
                    }

                    if let Some(message) = &label.message {
                        interop = interop.message(message.clone());
                    }

                    diagnostic = diagnostic.label(interop);
                }

                Err(error) => {
                    diagnostic = diagnostic.note(format!(
                        "annotate-snippets source label could not be resolved: {error}"
                    ));
                }
            }
        }

        diagnostic
    }

    pub fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        self.to_interop_diagnostic().to_diagprint(reporter)
    }

    fn resolve_label(
        &self,
        label: &AnnotateSnippetsLabel,
    ) -> Result<ResolvedLabel, AnnotateSnippetsBridgeError> {
        let source = self.sources.get(&label.file).ok_or_else(|| {
            AnnotateSnippetsBridgeError::MissingSource {
                file: label.file.clone(),
            }
        })?;

        if label.range.start > label.range.end || label.range.end > source.len() {
            return Err(AnnotateSnippetsBridgeError::InvalidSpan {
                file: label.file.clone(),
                start: label.range.start,
                end: label.range.end,
                source_len: source.len(),
            });
        }

        if !source.is_char_boundary(label.range.start) || !source.is_char_boundary(label.range.end)
        {
            return Err(AnnotateSnippetsBridgeError::InvalidUtf8Boundary {
                file: label.file.clone(),
                start: label.range.start,
                end: label.range.end,
            });
        }

        let prefix = &source[..label.range.start];

        let line = prefix
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            .saturating_add(1);

        let line_start = prefix.rfind('\n').map(|offset| offset + 1).unwrap_or(0);

        let column = source[line_start..label.range.start]
            .chars()
            .count()
            .saturating_add(1);

        let selected = &source[label.range.clone()];

        let length = if selected.is_empty() || selected.contains('\n') || selected.contains('\r') {
            None
        } else {
            let chars = selected.chars().count();
            (chars > 0).then_some(chars)
        };

        Ok(ResolvedLabel {
            line: saturating_u32(line),
            column: saturating_u32(column),
            length,
        })
    }
}

impl InteropDiagnosticSource for AnnotateSnippetsBridge {
    fn to_interop_diagnostic(&self) -> InteropDiagnostic {
        AnnotateSnippetsBridge::to_interop_diagnostic(self)
    }
}

impl SourceProvider for AnnotateSnippetsBridge {
    fn populate_source_cache(&self, cache: &SourceCache) {
        for (file, source) in &self.sources {
            cache.insert(file.clone(), source.clone());
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedLabel {
    line: u32,
    column: u32,
    length: Option<usize>,
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn level_from_severity(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Fatal | Severity::Error => Level::ERROR,
        Severity::Warning => Level::WARNING,
        Severity::Info | Severity::Debug | Severity::Trace => Level::INFO,
    }
}
