//! Adapter from `codespan-reporting` diagnostics into `diagprint`.
//!
//! `codespan-reporting` separates diagnostics from source storage:
//!
//! - `Diagnostic<FileId>` owns severity, code, message, labels, and notes.
//! - `Files` resolves a `FileId` into source names and locations.
//!
//! This adapter consumes both structures directly. It does not render a
//! codespan diagnostic and then parse the resulting terminal text.
//!
//! No `diagprint` suggestions, edits, or commands are fabricated from
//! codespan diagnostics.

use crate::{Diagnostic, Reporter, Severity};
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
    /// Converts a codespan diagnostic using its associated source-file
    /// database to resolve file names and byte positions.
    ///
    /// Labels which cannot be resolved safely are retained as diagnostic
    /// notes instead of causing the entire conversion to fail.
    fn to_diagprint<'a, F>(&self, reporter: &Reporter, files: &'a F) -> Diagnostic
    where
        FileId: 'a + Copy + PartialEq,
        F: Files<'a, FileId = FileId>;
}

impl<FileId> CodespanDiagnosticExt<FileId> for CodespanDiagnostic<FileId> {
    fn to_diagprint<'a, F>(&self, reporter: &Reporter, files: &'a F) -> Diagnostic
    where
        FileId: 'a + Copy + PartialEq,
        F: Files<'a, FileId = FileId>,
    {
        convert_diagnostic(reporter, files, self)
    }
}

fn convert_diagnostic<'a, F>(
    reporter: &Reporter,
    files: &'a F,
    source: &CodespanDiagnostic<F::FileId>,
) -> Diagnostic
where
    F: Files<'a>,
{
    let mut diagnostic = reporter.diagnostic(
        severity_from_codespan(source.severity),
        source.message.clone(),
    );

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
                diagnostic = diagnostic.label(
                    resolved.file,
                    resolved.line,
                    Some(resolved.column),
                    resolved.width,
                    resolved.message,
                );
            }

            Err(reason) => {
                diagnostic = diagnostic.note(unresolved_label_note(label, &reason));
            }
        }
    }

    /*
     * diagprint's current core Label type does not distinguish primary and
     * secondary source labels. Preserve that information explicitly rather
     * than silently discarding it.
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

    Ok(ResolvedLabel {
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
