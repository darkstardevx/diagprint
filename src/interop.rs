//! Dependency-free diagnostic interoperability protocol.
//!
//! External diagnostic producers can translate their structured data into
//! [`InteropDiagnostic`] without depending on diagprint's renderer or
//! remediation internals.
//!
//! This protocol deliberately contains diagnostic metadata only.
//!
//! It does not carry automatic edits or executable commands. Ecosystems with
//! machine-applicable remediation need a dedicated adapter which can prove the
//! stronger safety invariants required by [`crate::Fixer`] and
//! [`crate::FixPlan`].

use crate::{
    Applicability, Cause, Diagnostic, DocumentationLink, LabelKind, Reporter, Severity, Suggestion,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteropLabel {
    pub kind: LabelKind,

    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
    pub length: Option<usize>,

    pub message: Option<String>,
}

impl InteropLabel {
    pub fn primary(file: impl Into<String>, line: u32) -> Self {
        Self::new(LabelKind::Primary, file, line)
    }

    pub fn secondary(file: impl Into<String>, line: u32) -> Self {
        Self::new(LabelKind::Secondary, file, line)
    }

    pub fn new(kind: LabelKind, file: impl Into<String>, line: u32) -> Self {
        Self {
            kind,
            file: file.into(),
            line,
            column: None,
            length: None,
            message: None,
        }
    }

    pub fn column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    pub fn length(mut self, length: usize) -> Self {
        self.length = Some(length);
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

/// Neutral, owned representation of a diagnostic produced by another
/// diagnostic ecosystem.
///
/// The source producer resolves any ecosystem-specific handles, byte spans, or
/// file IDs before constructing this value.
#[derive(Debug, Clone)]
pub struct InteropDiagnostic {
    pub severity: Severity,

    pub code: Option<String>,
    pub message: String,
    pub help: Option<String>,

    pub labels: Vec<InteropLabel>,
    pub notes: Vec<String>,

    pub cause: Option<Cause>,

    pub documentation: Vec<DocumentationLink>,

    pub related: Vec<InteropDiagnostic>,
}

impl InteropDiagnostic {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,

            code: None,
            message: message.into(),
            help: None,

            labels: Vec::new(),
            notes: Vec::new(),

            cause: None,

            documentation: Vec::new(),

            related: Vec::new(),
        }
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
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

    pub fn label(mut self, label: InteropLabel) -> Self {
        self.labels.push(label);
        self
    }

    pub fn cause(mut self, cause: Cause) -> Self {
        self.cause = Some(cause);
        self
    }

    pub fn documentation(mut self, link: DocumentationLink) -> Self {
        self.documentation.push(link);
        self
    }

    pub fn related(mut self, diagnostic: InteropDiagnostic) -> Self {
        self.related.push(diagnostic);
        self
    }

    /// Converts this diagnostic into diagprint's core representation.
    ///
    /// Related diagnostics remain available through
    /// [`InteropDiagnostic::to_diagprint_tree`]. When converting only the
    /// primary diagnostic, the number of related diagnostics is retained as a
    /// note so their existence is not silently lost.
    pub fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        let mut diagnostic = reporter.diagnostic(self.severity, self.message.clone());

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
            diagnostic = diagnostic.label_with_kind(
                label.kind,
                label.file.clone(),
                label.line,
                label.column,
                label.length,
                label.message.clone(),
            );
        }

        if let Some(cause) = &self.cause {
            diagnostic = diagnostic.cause_chain(cause.clone());
        }

        if !self.documentation.is_empty() {
            let mut suggestion = Suggestion::new("Read diagnostic documentation")
                .explanation("Imported from structured diagnostic metadata.")
                .applicability(Applicability::Manual);

            for link in &self.documentation {
                suggestion = suggestion.documentation(link.clone());
            }

            diagnostic = diagnostic.suggestion(suggestion);
        }

        if !self.related.is_empty() {
            diagnostic = diagnostic.note(format!("related diagnostics: {}", self.related.len()));
        }

        diagnostic
    }

    pub fn to_diagprint_tree(&self, reporter: &Reporter) -> DiagnosticTree {
        DiagnosticTree {
            diagnostic: self.to_diagprint(reporter),

            related: self
                .related
                .iter()
                .map(|related| related.to_diagprint_tree(reporter))
                .collect(),
        }
    }
}

/// Common tree representation for diagnostic ecosystems which expose related
/// diagnostics separately from a primary diagnostic.
#[derive(Debug, Clone)]
pub struct DiagnosticTree {
    pub diagnostic: Diagnostic,

    pub related: Vec<DiagnosticTree>,
}

impl DiagnosticTree {
    pub fn related_count(&self) -> usize {
        self.related.len()
    }

    pub fn total_diagnostics(&self) -> usize {
        1 + self
            .related
            .iter()
            .map(Self::total_diagnostics)
            .sum::<usize>()
    }
}

/// Implement this trait on a custom compiler, linter, parser, language tool,
/// or other structured diagnostic producer.
///
/// External source databases or ecosystem-specific span handles should be
/// resolved before returning [`InteropDiagnostic`].
pub trait InteropDiagnosticSource {
    fn to_interop_diagnostic(&self) -> InteropDiagnostic;
}

/// Convenience conversion methods for any custom producer implementing
/// [`InteropDiagnosticSource`].
pub trait InteropDiagnosticSourceExt: InteropDiagnosticSource {
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        self.to_interop_diagnostic().to_diagprint(reporter)
    }

    fn to_diagprint_tree(&self, reporter: &Reporter) -> DiagnosticTree {
        self.to_interop_diagnostic().to_diagprint_tree(reporter)
    }
}

impl<T> InteropDiagnosticSourceExt for T where T: InteropDiagnosticSource + ?Sized {}
