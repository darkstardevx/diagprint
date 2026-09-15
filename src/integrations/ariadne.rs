//! Structured bridge between Ariadne and diagprint.
//!
//! Ariadne intentionally keeps the internal fields of finished `Report` and
//! `Label` values private. Its public API is focused on building and rendering
//! reports rather than extracting their structured contents.
//!
//! For that reason this module does not parse Ariadne's pretty terminal output
//! and does not depend on private implementation details.
//!
//! Instead, [`AriadneBridge`] owns the structured diagnostic metadata once and
//! can produce both:
//!
//! - an Ariadne `Report`; and
//! - diagprint's dependency-neutral [`crate::InteropDiagnostic`].
//!
//! No remediation edits or executable commands are inferred.

use crate::{
    Diagnostic, InteropDiagnostic, InteropDiagnosticSource, InteropLabel, LabelKind, Reporter,
    Severity,
};
use ::ariadne::{Label as AriadneLabel, Report as AriadneReport, ReportKind};
use std::{collections::BTreeMap, error::Error, fmt, ops::Range};

/// Owned span type used by reports emitted through [`AriadneBridge`].
pub type AriadneOwnedSpan = (String, Range<usize>);

/// Source identifier and zero-based character range used by Ariadne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AriadneSpan {
    pub file: String,
    pub range: Range<usize>,
}

impl AriadneSpan {
    pub fn new(file: impl Into<String>, range: Range<usize>) -> Self {
        Self {
            file: file.into(),
            range,
        }
    }

    fn owned(&self) -> AriadneOwnedSpan {
        (self.file.clone(), self.range.clone())
    }
}

/// A structured source label shared by the Ariadne and diagprint views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AriadneBridgeLabel {
    pub kind: LabelKind,
    pub span: AriadneSpan,
    pub message: Option<String>,

    /// Ariadne-specific rendering order.
    ///
    /// This is a presentation hint and is not added to diagprint's generic
    /// interop protocol.
    pub order: Option<i32>,

    /// Ariadne-specific overlap priority.
    ///
    /// This is a presentation hint and is not added to diagprint's generic
    /// interop protocol.
    pub priority: Option<i32>,
}

impl AriadneBridgeLabel {
    pub fn primary(file: impl Into<String>, range: Range<usize>) -> Self {
        Self::new(LabelKind::Primary, file, range)
    }

    pub fn secondary(file: impl Into<String>, range: Range<usize>) -> Self {
        Self::new(LabelKind::Secondary, file, range)
    }

    pub fn new(kind: LabelKind, file: impl Into<String>, range: Range<usize>) -> Self {
        Self {
            kind,
            span: AriadneSpan::new(file, range),
            message: None,
            order: None,
            priority: None,
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = Some(order);
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
        self
    }
}

/// Validation failure while producing an Ariadne report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AriadneBridgeError {
    MissingSource {
        file: String,
    },

    InvalidSpan {
        file: String,
        start: usize,
        end: usize,
        source_len: usize,
    },
}

impl fmt::Display for AriadneBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource { file } => {
                write!(f, "Ariadne source {file:?} was not registered")
            }

            Self::InvalidSpan {
                file,
                start,
                end,
                source_len,
            } => write!(
                f,
                "Ariadne character range {start}..{end} for {file:?} \
                 is invalid for a source containing {source_len} characters"
            ),
        }
    }
}

impl Error for AriadneBridgeError {}

/// Structured diagnostic which can be emitted through both Ariadne and
/// diagprint.
///
/// Ariadne spans are zero-based character offsets, not byte offsets.
#[derive(Debug, Clone)]
pub struct AriadneBridge {
    severity: Severity,
    code: Option<String>,
    message: String,
    help: Option<String>,
    notes: Vec<String>,

    report_span: AriadneSpan,
    labels: Vec<AriadneBridgeLabel>,
    sources: BTreeMap<String, String>,
}

impl AriadneBridge {
    pub fn new(
        severity: Severity,
        message: impl Into<String>,
        file: impl Into<String>,
        report_range: Range<usize>,
    ) -> Self {
        Self {
            severity,
            code: None,
            message: message.into(),
            help: None,
            notes: Vec::new(),

            report_span: AriadneSpan::new(file, report_range),
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

    pub fn label(mut self, label: AriadneBridgeLabel) -> Self {
        self.labels.push(label);
        self
    }

    pub fn primary_label(
        self,
        file: impl Into<String>,
        range: Range<usize>,
        message: impl Into<String>,
    ) -> Self {
        self.label(AriadneBridgeLabel::primary(file, range).message(message))
    }

    pub fn secondary_label(
        self,
        file: impl Into<String>,
        range: Range<usize>,
        message: impl Into<String>,
    ) -> Self {
        self.label(AriadneBridgeLabel::secondary(file, range).message(message))
    }

    /// Returns owned source pairs suitable for `ariadne::sources(...)`.
    pub fn ariadne_sources(&self) -> Vec<(String, String)> {
        self.sources
            .iter()
            .map(|(file, source)| (file.clone(), source.clone()))
            .collect()
    }

    /// Validates every character span against its registered source.
    pub fn validate(&self) -> Result<(), AriadneBridgeError> {
        self.resolve_location(&self.report_span)?;

        for label in &self.labels {
            self.resolve_location(&label.span)?;
        }

        Ok(())
    }

    /// Builds an Ariadne report from the bridge's structured metadata.
    pub fn to_ariadne_report(
        &self,
    ) -> Result<AriadneReport<'static, AriadneOwnedSpan>, AriadneBridgeError> {
        self.validate()?;

        let span = self.report_span.owned();

        let mut builder = if let Some(code) = &self.code {
            AriadneReport::build(report_kind(self.severity), span).with_code(code.clone())
        } else {
            AriadneReport::build(report_kind(self.severity), span)
        };

        builder.set_message(self.message.clone());

        for note in &self.notes {
            builder.add_note(note.clone());
        }

        if let Some(help) = &self.help {
            builder.add_help(help.clone());
        }

        for label in &self.labels {
            let mut ariadne_label = AriadneLabel::new(label.span.owned());

            if let Some(message) = &label.message {
                ariadne_label = ariadne_label.with_message(message.clone());
            }

            if let Some(order) = label.order {
                ariadne_label = ariadne_label.with_order(order);
            }

            if let Some(priority) = label.priority {
                ariadne_label = ariadne_label.with_priority(priority);
            }

            builder.add_label(ariadne_label);
        }

        Ok(builder.finish())
    }

    /// Converts the bridge into diagprint's generic interoperability protocol.
    ///
    /// A bridge source problem is retained as a diagnostic note rather than
    /// causing a panic or inventing a location.
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

        let report_span_has_primary_label = self
            .labels
            .iter()
            .any(|label| label.kind == LabelKind::Primary && label.span == self.report_span);

        if !report_span_has_primary_label {
            match self.resolve_interop_label(LabelKind::Primary, &self.report_span, None) {
                Ok(label) => {
                    diagnostic = diagnostic.label(label);
                }

                Err(error) => {
                    diagnostic = diagnostic.note(format!(
                        "Ariadne report span could not be resolved: {error}"
                    ));
                }
            }
        }

        for label in &self.labels {
            match self.resolve_interop_label(label.kind, &label.span, label.message.as_deref()) {
                Ok(label) => {
                    diagnostic = diagnostic.label(label);
                }

                Err(error) => {
                    diagnostic = diagnostic.note(format!(
                        "Ariadne source label could not be resolved: {error}"
                    ));
                }
            }
        }

        diagnostic
    }

    pub fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        self.to_interop_diagnostic().to_diagprint(reporter)
    }

    fn resolve_interop_label(
        &self,
        kind: LabelKind,
        span: &AriadneSpan,
        message: Option<&str>,
    ) -> Result<InteropLabel, AriadneBridgeError> {
        let resolved = self.resolve_location(span)?;

        let mut label =
            InteropLabel::new(kind, span.file.clone(), resolved.line).column(resolved.column);

        if let Some(length) = resolved.length {
            label = label.length(length);
        }

        if let Some(message) = message {
            label = label.message(message);
        }

        Ok(label)
    }

    fn resolve_location(&self, span: &AriadneSpan) -> Result<ResolvedLocation, AriadneBridgeError> {
        let source =
            self.sources
                .get(&span.file)
                .ok_or_else(|| AriadneBridgeError::MissingSource {
                    file: span.file.clone(),
                })?;

        let source_len = source.chars().count();

        if span.range.start > span.range.end || span.range.end > source_len {
            return Err(AriadneBridgeError::InvalidSpan {
                file: span.file.clone(),
                start: span.range.start,
                end: span.range.end,
                source_len,
            });
        }

        let (line, column) = line_column_at(source, span.range.start);

        let text: String = source
            .chars()
            .skip(span.range.start)
            .take(span.range.end.saturating_sub(span.range.start))
            .collect();

        let length = if text.is_empty() || text.contains('\n') || text.contains('\r') {
            None
        } else {
            Some(span.range.end - span.range.start)
        };

        Ok(ResolvedLocation {
            line,
            column,
            length,
        })
    }
}

impl InteropDiagnosticSource for AriadneBridge {
    fn to_interop_diagnostic(&self) -> InteropDiagnostic {
        AriadneBridge::to_interop_diagnostic(self)
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedLocation {
    line: u32,
    column: u32,
    length: Option<usize>,
}

fn line_column_at(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1usize;
    let mut column = 1usize;

    for ch in source.chars().take(offset) {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (saturating_u32(line), saturating_u32(column))
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn report_kind(severity: Severity) -> ReportKind<'static> {
    match severity {
        Severity::Fatal | Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
        Severity::Info | Severity::Debug | Severity::Trace => ReportKind::Advice,
    }
}
