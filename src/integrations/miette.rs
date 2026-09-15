//! Adapter from `miette`'s diagnostic protocol into `diagprint`.
//!
//! This adapter consumes structured metadata from [`miette::Diagnostic`]
//! directly. It never parses miette's human-readable rendering.
//!
//! Preserved metadata includes:
//!
//! - message;
//! - severity;
//! - diagnostic code;
//! - help;
//! - documentation URL;
//! - source labels;
//! - diagnostic/source chains;
//! - nested related diagnostics.
//!
//! Miette diagnostics do not implicitly become `diagprint` fixes.
//! Documentation URLs are imported as manual suggestions only.

use crate::{Cause, Diagnostic, DocumentationLink, Reporter, Severity, Suggestion};
use ::miette::{
    Diagnostic as MietteDiagnostic, LabeledSpan, Severity as MietteSeverity, SpanContents,
};
use std::error::Error;

const MAX_CAUSE_DEPTH: usize = 64;
const MAX_RELATED_DEPTH: usize = 64;

/// A recursively preserved miette diagnostic plus its related diagnostics.
///
/// This remains as a compatibility alias while the actual tree representation
/// now lives in diagprint's core interoperability layer.
pub type MietteDiagnosticTree = crate::DiagnosticTree;

/// Extension methods for types implementing [`miette::Diagnostic`].
pub trait MietteDiagnosticExt: MietteDiagnostic {
    /// Converts only this diagnostic.
    ///
    /// Use [`MietteDiagnosticExt::to_diagprint_tree`] when miette
    /// `related()` diagnostics must also be retained.
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        convert_diagnostic(reporter, self)
    }

    /// Converts this diagnostic and recursively preserves miette
    /// `related()` diagnostics.
    fn to_diagprint_tree(&self, reporter: &Reporter) -> MietteDiagnosticTree {
        convert_tree(reporter, self, 0)
    }
}

impl<T> MietteDiagnosticExt for T where T: MietteDiagnostic + ?Sized {}

/// Extension methods for [`miette::Report`].
///
/// `miette::Report` dereferences to a diagnostic but does not itself
/// implement the `Diagnostic` trait. A dedicated extension trait keeps the
/// ergonomic `report.to_diagprint(...)` form available.
pub trait MietteReportExt {
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic;

    fn to_diagprint_tree(&self, reporter: &Reporter) -> MietteDiagnosticTree;
}

impl MietteReportExt for ::miette::Report {
    fn to_diagprint(&self, reporter: &Reporter) -> Diagnostic {
        convert_diagnostic(reporter, &**self)
    }

    fn to_diagprint_tree(&self, reporter: &Reporter) -> MietteDiagnosticTree {
        convert_tree(reporter, &**self, 0)
    }
}

fn convert_diagnostic<D>(reporter: &Reporter, source: &D) -> Diagnostic
where
    D: MietteDiagnostic + ?Sized,
{
    let mut diagnostic =
        reporter.diagnostic(severity_from_miette(source.severity()), source.to_string());

    if let Some(code) = source.code() {
        diagnostic = diagnostic.code(code.to_string());
    }

    if let Some(help) = source.help() {
        diagnostic = diagnostic.help(help.to_string());
    }

    diagnostic = attach_labels(diagnostic, source);

    diagnostic = attach_cause_chain(diagnostic, source);

    if let Some(url) = source.url() {
        let url = url.to_string();

        diagnostic = diagnostic.suggestion(
            Suggestion::new("Read diagnostic documentation")
                .explanation(
                    "Imported from the documentation URL supplied by the miette diagnostic.",
                )
                .documentation(DocumentationLink::new(
                    "miette diagnostic documentation",
                    url,
                )),
        );
    }

    if let Some(related) = source.related() {
        let count = related.count();

        if count > 0 {
            diagnostic = diagnostic.note(format!("miette related diagnostics: {count}"));
        }
    }

    diagnostic
}

fn convert_tree<D>(reporter: &Reporter, source: &D, depth: usize) -> MietteDiagnosticTree
where
    D: MietteDiagnostic + ?Sized,
{
    let mut diagnostic = convert_diagnostic(reporter, source);

    if depth >= MAX_RELATED_DEPTH {
        if source.related().is_some() {
            diagnostic = diagnostic.note(format!(
                "miette related diagnostic expansion stopped at depth {MAX_RELATED_DEPTH}"
            ));
        }

        return MietteDiagnosticTree {
            diagnostic,
            related: Vec::new(),
        };
    }

    let related = source
        .related()
        .map(|related| {
            related
                .map(|child| convert_tree(reporter, child, depth + 1))
                .collect()
        })
        .unwrap_or_default();

    MietteDiagnosticTree {
        diagnostic,
        related,
    }
}

fn severity_from_miette(severity: Option<MietteSeverity>) -> Severity {
    match severity.unwrap_or(MietteSeverity::Error) {
        MietteSeverity::Error => Severity::Error,

        MietteSeverity::Warning => Severity::Warning,

        MietteSeverity::Advice => Severity::Info,
    }
}

fn attach_labels<D>(mut diagnostic: Diagnostic, source: &D) -> Diagnostic
where
    D: MietteDiagnostic + ?Sized,
{
    let Some(labels) = source.labels() else {
        return diagnostic;
    };

    let source_code = source.source_code();

    for label in labels {
        let Some(source_code) = source_code else {
            diagnostic = diagnostic.note(unresolved_label_note(
                &label,
                "diagnostic does not expose SourceCode",
            ));

            continue;
        };

        match source_code.read_span(label.inner(), 0, 0) {
            Ok(contents) => {
                let file = contents.name().unwrap_or("<miette source>").to_owned();

                let (line, column, width) = resolve_label_position(contents.as_ref(), &label);

                diagnostic = diagnostic.label(
                    file,
                    line,
                    Some(column),
                    width,
                    label.label().map(ToOwned::to_owned),
                );
            }

            Err(error) => {
                diagnostic = diagnostic.note(unresolved_label_note(&label, &error.to_string()));
            }
        }
    }

    diagnostic
}

fn resolve_label_position<'a>(
    contents: &(dyn SpanContents<'a> + 'a),
    label: &LabeledSpan,
) -> (u32, u32, Option<usize>) {
    let fallback = (
        one_based_u32(contents.line()),
        one_based_u32(contents.column()),
        (!label.is_empty()).then_some(label.len()),
    );

    let base_offset = contents.span().offset();

    let Some(local_start) = label.offset().checked_sub(base_offset) else {
        return fallback;
    };

    let Some(local_end) = local_start.checked_add(label.len()) else {
        return fallback;
    };

    let data = contents.data();

    let Some(prefix_bytes) = data.get(..local_start) else {
        return fallback;
    };

    let Some(label_bytes) = data.get(local_start..local_end) else {
        return fallback;
    };

    let Ok(prefix) = std::str::from_utf8(prefix_bytes) else {
        return fallback;
    };

    let line_breaks = prefix.bytes().filter(|byte| *byte == b'\n').count();

    let line_zero_based = contents.line().saturating_add(line_breaks);

    let column_zero_based = if let Some(last_newline) = prefix.rfind('\n') {
        unicode_width::UnicodeWidthStr::width(&prefix[last_newline + 1..])
    } else {
        contents
            .column()
            .saturating_add(unicode_width::UnicodeWidthStr::width(prefix))
    };

    let width = if label.is_empty() || label_bytes.contains(&b'\n') {
        None
    } else {
        std::str::from_utf8(label_bytes)
            .ok()
            .map(unicode_width::UnicodeWidthStr::width)
            .filter(|width| *width > 0)
    };

    (
        one_based_u32(line_zero_based),
        one_based_u32(column_zero_based),
        width,
    )
}

fn unresolved_label_note(label: &LabeledSpan, reason: &str) -> String {
    let end = label.offset().saturating_add(label.len());

    match label.label() {
        Some(message) => {
            format!(
                "miette source span bytes {}..{} ({message:?}) could not be resolved: {reason}",
                label.offset(),
                end,
            )
        }

        None => {
            format!(
                "miette source span bytes {}..{} could not be resolved: {reason}",
                label.offset(),
                end,
            )
        }
    }
}

fn attach_cause_chain<D>(diagnostic: Diagnostic, source: &D) -> Diagnostic
where
    D: MietteDiagnostic + ?Sized,
{
    let cause = if let Some(source) = source.diagnostic_source() {
        Some(cause_from_miette(source, 1))
    } else {
        source.source().map(|source| cause_from_error(source, 1))
    };

    match cause {
        Some(cause) => diagnostic.cause_chain(cause),

        None => diagnostic,
    }
}

fn cause_from_miette(source: &dyn MietteDiagnostic, depth: usize) -> Cause {
    let mut cause = Cause::new(source.to_string());

    if depth >= MAX_CAUSE_DEPTH {
        return cause;
    }

    if let Some(next) = source.diagnostic_source() {
        cause = cause.caused_by(cause_from_miette(next, depth + 1));

        return cause;
    }

    if let Some(next) = source.source() {
        cause = cause.caused_by(cause_from_error(next, depth + 1));
    }

    cause
}

fn cause_from_error(source: &(dyn Error + 'static), depth: usize) -> Cause {
    let mut cause = Cause::new(source.to_string());

    if depth >= MAX_CAUSE_DEPTH {
        return cause;
    }

    if let Some(next) = source.source() {
        cause = cause.caused_by(cause_from_error(next, depth + 1));
    }

    cause
}

fn one_based_u32(zero_based: usize) -> u32 {
    u32::try_from(zero_based.saturating_add(1)).unwrap_or(u32::MAX)
}
