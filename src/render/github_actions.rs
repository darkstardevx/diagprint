use super::Renderer;
use crate::{Diagnostic, Label, LabelKind, Severity};

/// Renders diagnostics as GitHub Actions workflow-command annotations.
///
/// Primary labels retain the diagnostic severity:
///
/// - trace/debug/info -> `notice`
/// - warning -> `warning`
/// - error/fatal -> `error`
///
/// Secondary labels are emitted as `notice` annotations so related source
/// locations remain visible without being reported as additional failures.
#[derive(Debug, Default, Clone, Copy)]
pub struct GithubActionsRenderer;

impl GithubActionsRenderer {
    fn command(severity: Severity) -> &'static str {
        match severity {
            Severity::Trace | Severity::Debug | Severity::Info => "notice",
            Severity::Warning => "warning",
            Severity::Error | Severity::Fatal => "error",
        }
    }

    fn escape_data(value: &str) -> String {
        value
            .replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    }

    fn escape_property(value: &str) -> String {
        value
            .replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
            .replace(':', "%3A")
            .replace(',', "%2C")
    }

    fn title(diagnostic: &Diagnostic, label: Option<&Label>) -> Option<String> {
        match label.map(|label| label.kind) {
            Some(LabelKind::Secondary) => Some(
                diagnostic
                    .code
                    .as_ref()
                    .map(|code| format!("{code} related"))
                    .unwrap_or_else(|| "related location".to_owned()),
            ),

            _ => diagnostic.code.clone(),
        }
    }

    fn message(diagnostic: &Diagnostic, label: Option<&Label>) -> String {
        if label.is_some_and(|label| label.kind == LabelKind::Secondary) {
            return label
                .and_then(|label| label.message.clone())
                .unwrap_or_else(|| format!("Related location for {}", diagnostic.message));
        }

        let mut message = diagnostic.message.clone();

        if let Some(label_message) = label.and_then(|label| label.message.as_deref()) {
            message.push('\n');
            message.push_str(label_message);
        }

        for note in &diagnostic.notes {
            message.push_str("\nnote: ");
            message.push_str(note);
        }

        if let Some(help) = &diagnostic.help {
            message.push_str("\nhelp: ");
            message.push_str(help);
        }

        message
    }

    fn render_annotation(diagnostic: &Diagnostic, label: Option<&Label>) -> String {
        let command = match label.map(|label| label.kind) {
            Some(LabelKind::Secondary) => "notice",
            _ => Self::command(diagnostic.severity),
        };

        let mut properties = Vec::new();

        if let Some(label) = label {
            let location = &label.location;

            properties.push(format!("file={}", Self::escape_property(&location.file)));

            properties.push(format!("line={}", location.line));

            if let Some(column) = location.column {
                properties.push(format!("col={column}"));

                if let Some(length) = label.length {
                    if length > 0 {
                        let length = u32::try_from(length).unwrap_or(u32::MAX);

                        let end_column = column.saturating_add(length.saturating_sub(1));

                        properties.push(format!("endColumn={end_column}"));
                    }
                }
            }
        }

        if let Some(title) = Self::title(diagnostic, label) {
            properties.push(format!("title={}", Self::escape_property(&title)));
        }

        let message = Self::escape_data(&Self::message(diagnostic, label));

        if properties.is_empty() {
            format!("::{command}::{message}")
        } else {
            format!("::{command} {}::{message}", properties.join(","))
        }
    }
}

impl Renderer for GithubActionsRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        if diagnostic.labels.is_empty() {
            return Self::render_annotation(diagnostic, None);
        }

        diagnostic
            .labels
            .iter()
            .map(|label| Self::render_annotation(diagnostic, Some(label)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
