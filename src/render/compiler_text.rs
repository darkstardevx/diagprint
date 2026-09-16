use super::Renderer;
use crate::{Diagnostic, LabelKind, Severity};

/// Compact compiler/editor-oriented diagnostic renderer.
///
/// Output follows a deliberately simple location-first grammar:
///
/// ~~~text
/// path:line:column: severity[code]: message
/// ~~~
///
/// The representation contains no ANSI styling or volatile runtime metadata,
/// making it suitable for editor quick-fix parsers, CI logs, grep, and pipes.
#[derive(Debug, Default, Clone, Copy)]
pub struct CompilerTextRenderer;

impl Renderer for CompilerTextRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        let label = diagnostic
            .labels
            .iter()
            .find(|label| label.kind == LabelKind::Primary)
            .or_else(|| diagnostic.labels.first());

        let (file, line, column) = match label {
            Some(label) => (
                single_line(&label.location.file),
                label.location.line,
                label.location.column.unwrap_or(1),
            ),

            None => ("<unknown>".to_owned(), 0, 0),
        };

        let severity = severity_token(diagnostic.severity);

        let code = diagnostic
            .code
            .as_deref()
            .map(single_line)
            .map(|code| format!("[{code}]"))
            .unwrap_or_default();

        let message = single_line(&diagnostic.message);

        format!("{file}:{line}:{column}: {severity}{code}: {message}")
    }
}

fn severity_token(severity: Severity) -> &'static str {
    match severity {
        Severity::Trace => "trace",
        Severity::Debug => "debug",
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "fatal",
    }
}

fn single_line(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            other => other,
        })
        .collect()
}
