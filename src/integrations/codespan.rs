//! Adapter from `codespan-reporting` diagnostics into `diagprint`.
//!
//! `codespan-reporting` separates diagnostics from source storage:
//!
//! - `Diagnostic<FileId>` owns severity, code, message, labels, and notes.
//! - `Files` resolves a `FileId` into source names and locations.
//!
//! This adapter resolves that ecosystem-specific structure into diagprint's
//! dependency-free [`crate::InteropDiagnostic`] protocol.
//!
//! No suggestions, edits, or commands are fabricated from codespan
//! diagnostics.

use crate::{Diagnostic, InteropDiagnostic, InteropLabel, LabelKind, Reporter, Severity};
use ::codespan_reporting::{
    diagnostic::{
        Diagnostic as CodespanDiagnostic, Label as CodespanLabel, LabelStyle,
        Severity as CodespanSeverity,
    },
    files::Files,
};
use unicode_width::UnicodeWidthStr;

/// Extension methods for `codespan-reporting` diagnostics.
pub trait CodespanDiagnosticExt<FileId> {
    /// Resolves the codespan diagnostic into diagprint's generic interop
    /// protocol.
    fn to_interop_diagnostic<'a, F>(&self, files: &'a F) -> InteropDiagnostic
    where
        FileId: 'a + Copy + PartialEq,
        F: Files<'a, FileId = FileId>;

    /// Converts the diagnostic directly into diagprint.
    fn to_diagprint<'a, F>(&self, reporter: &Reporter, files: &'a F) -> Diagnostic
    where
        FileId: 'a + Copy + PartialEq,
        F: Files<'a, FileId = FileId>,
    {
        self.to_interop_diagnostic(files).to_diagprint(reporter)
    }
}

impl<FileId> CodespanDiagnosticExt<FileId> for CodespanDiagnostic<FileId> {
    fn to_interop_diagnostic<'a, F>(&self, files: &'a F) -> InteropDiagnostic
    where
        FileId: 'a + Copy + PartialEq,
        F: Files<'a, FileId = FileId>,
    {
        convert_diagnostic(files, self)
    }
}

fn convert_diagnostic<'a, F>(
    files: &'a F,
    source: &CodespanDiagnostic<F::FileId>,
) -> InteropDiagnostic
where
    F: Files<'a>,
{
    let mut diagnostic = InteropDiagnostic::new(source.message.clone())
        .severity(severity_from_codespan(source.severity));

    if let Some(code) = &source.code {
        diagnostic = diagnostic.code(code.clone());
    }

    for note in &source.notes {
        diagnostic = diagnostic.note(note.clone());
    }

    let mut secondary_labels = 0usize;

    for label in &source.labels {
        if label.style == LabelStyle::Secondary {
            secondary_labels += 1;
        }

        match resolve_label(files, label) {
            Ok(resolved) => {
                let mut interop_label =
                    InteropLabel::new(resolved.kind, resolved.file, resolved.line)
                        .column(resolved.column);

                if let Some(width) = resolved.width {
                    interop_label = interop_label.length(width);
                }

                if let Some(message) = resolved.message {
                    interop_label = interop_label.message(message);
                }

                diagnostic = diagnostic.label(interop_label);
            }

            Err(reason) => {
                diagnostic = diagnostic.note(unresolved_label_note(label, &reason));
            }
        }
    }

    /*
     * Retain the compatibility note from the original codespan adapter while
     * also preserving secondary-ness structurally through LabelKind.
     */
    if secondary_labels > 0 {
        diagnostic = diagnostic.note(format!(
            "codespan secondary source labels: {secondary_labels}"
        ));
    }

    diagnostic
}

#[derive(Debug)]
struct ResolvedLabel {
    kind: LabelKind,

    file: String,
    line: u32,
    column: u32,

    width: Option<usize>,
    message: Option<String>,
}

fn resolve_label<'a, F>(
    files: &'a F,
    label: &CodespanLabel<F::FileId>,
) -> Result<ResolvedLabel, String>
where
    F: Files<'a>,
{
    if label.range.start > label.range.end {
        return Err(format!(
            "invalid byte range {}..{}",
            label.range.start, label.range.end,
        ));
    }

    let name = files
        .name(label.file_id)
        .map_err(|error| format!("could not resolve file name: {error}"))?;

    let source = files
        .source(label.file_id)
        .map_err(|error| format!("could not resolve source text: {error}"))?;

    let source = source.as_ref();

    if label.range.end > source.len() {
        return Err(format!(
            "byte range {}..{} exceeds source length {}",
            label.range.start,
            label.range.end,
            source.len(),
        ));
    }

    if !source.is_char_boundary(label.range.start) || !source.is_char_boundary(label.range.end) {
        return Err(format!(
            "byte range {}..{} does not align to UTF-8 boundaries",
            label.range.start, label.range.end,
        ));
    }

    let line_index = files
        .line_index(label.file_id, label.range.start)
        .map_err(|error| format!("could not resolve line index: {error}"))?;

    let line_number = files
        .line_number(label.file_id, line_index)
        .map_err(|error| format!("could not resolve line number: {error}"))?;

    let column_number = files
        .column_number(label.file_id, line_index, label.range.start)
        .map_err(|error| format!("could not resolve column number: {error}"))?;

    let slice = &source[label.range.clone()];

    let width = if slice.is_empty() || slice.contains('\n') || slice.contains('\r') {
        None
    } else {
        let width = UnicodeWidthStr::width(slice);

        (width > 0).then_some(width)
    };

    let message = (!label.message.is_empty()).then(|| label.message.clone());

    let kind = match label.style {
        LabelStyle::Primary => LabelKind::Primary,

        LabelStyle::Secondary => LabelKind::Secondary,
    };

    Ok(ResolvedLabel {
        kind,

        file: name.to_string(),

        line: saturating_u32(line_number),

        column: saturating_u32(column_number),

        width,
        message,
    })
}

fn unresolved_label_note<FileId>(label: &CodespanLabel<FileId>, reason: &str) -> String {
    let style = match label.style {
        LabelStyle::Primary => "primary",

        LabelStyle::Secondary => "secondary",
    };

    if label.message.is_empty() {
        format!(
            "codespan {style} label bytes {}..{} could not be resolved: {reason}",
            label.range.start, label.range.end,
        )
    } else {
        format!(
            "codespan {style} label bytes {}..{} ({:?}) could not be resolved: {reason}",
            label.range.start, label.range.end, label.message,
        )
    }
}

fn severity_from_codespan(severity: CodespanSeverity) -> Severity {
    match severity {
        CodespanSeverity::Bug => Severity::Fatal,

        CodespanSeverity::Error => Severity::Error,

        CodespanSeverity::Warning => Severity::Warning,

        CodespanSeverity::Note | CodespanSeverity::Help => Severity::Info,
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
