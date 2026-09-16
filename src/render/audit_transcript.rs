use crate::{CanonicalizationError, DiagnosticReport, LabelKind, ReportDigest, Severity};

/// Deterministic, diff-friendly diagnostic audit transcript.
///
/// Audit transcripts deliberately exclude volatile runtime values such as
/// timestamps, process IDs, hostnames, session IDs, and report UUIDs.
///
/// The transcript is anchored to the canonical semantic [`ReportDigest`] and
/// sorts diagnostics deterministically before rendering.
#[derive(Debug, Default, Clone, Copy)]
pub struct AuditTranscriptRenderer;

impl AuditTranscriptRenderer {
    /// Renders a deterministic audit transcript for one report.
    pub fn render_report(
        &self,
        report: &DiagnosticReport,
    ) -> Result<String, CanonicalizationError> {
        let digest = report.digest()?;

        Ok(self.render_report_with_digest(report, digest))
    }

    pub(super) fn render_report_with_digest(
        &self,
        report: &DiagnosticReport,
        digest: ReportDigest,
    ) -> String {
        let mut report = report.clone();
        report.sort_deterministic();

        let counts = report.counts();

        let mut output = String::new();

        output.push_str("DIAGPRINT AUDIT TRANSCRIPT v1\n");
        output.push_str(&format!("report-digest: {}\n", digest.qualified(),));
        output.push_str(&format!("diagnostics: {}\n", report.len(),));
        output.push_str(&format!("failures: {}\n", counts.failures(),));

        output.push_str(&format!(
            "severity-counts: trace={} debug={} info={} warning={} error={} fatal={}\n",
            counts.trace, counts.debug, counts.info, counts.warning, counts.error, counts.fatal,
        ));

        for (index, diagnostic) in report.iter().enumerate() {
            output.push_str("\n---\n");

            output.push_str(&format!("diagnostic: {}\n", index + 1,));

            output.push_str(&format!(
                "fingerprint: {}\n",
                diagnostic.fingerprint().qualified(),
            ));

            output.push_str(&format!(
                "severity: {}\n",
                severity_token(diagnostic.severity),
            ));

            output.push_str("code: ");
            match &diagnostic.code {
                Some(code) => {
                    output.push_str(&escape_scalar(code));
                }
                None => output.push('-'),
            }
            output.push('\n');

            output.push_str("message: ");
            output.push_str(&escape_scalar(&diagnostic.message));
            output.push('\n');

            let mut labels = diagnostic
                .labels
                .iter()
                .map(|label| {
                    format!(
                        "{} file={} line={} column={} length={} message={}",
                        label_kind_token(label.kind),
                        escape_scalar(&label.location.file),
                        label.location.line,
                        label
                            .location
                            .column
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_owned()),
                        label
                            .length
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_owned()),
                        label
                            .message
                            .as_deref()
                            .map(escape_scalar)
                            .unwrap_or_else(|| "-".to_owned()),
                    )
                })
                .collect::<Vec<_>>();

            labels.sort();

            output.push_str(&format!("labels: {}\n", labels.len(),));

            for label in labels {
                output.push_str("  ");
                output.push_str(&label);
                output.push('\n');
            }

            let mut attributes = diagnostic
                .attributes
                .iter()
                .map(|attribute| {
                    format!(
                        "{}={}",
                        escape_scalar(&attribute.name),
                        escape_scalar(&attribute.value.to_string(),),
                    )
                })
                .collect::<Vec<_>>();

            attributes.sort();

            output.push_str(&format!("attributes: {}\n", attributes.len(),));

            for attribute in attributes {
                output.push_str("  ");
                output.push_str(&attribute);
                output.push('\n');
            }

            output.push_str(&format!(
                "causes: {}\n",
                diagnostic
                    .cause
                    .as_ref()
                    .map(|cause| cause.iter().count())
                    .unwrap_or(0),
            ));

            if let Some(cause) = &diagnostic.cause {
                for item in cause.iter() {
                    output.push_str("  ");
                    output.push_str(&escape_scalar(&item.message));
                    output.push('\n');
                }
            }

            output.push_str(&format!("notes: {}\n", diagnostic.notes.len(),));

            for note in &diagnostic.notes {
                output.push_str("  ");
                output.push_str(&escape_scalar(note));
                output.push('\n');
            }

            output.push_str("help: ");

            match &diagnostic.help {
                Some(help) => {
                    output.push_str(&escape_scalar(help));
                }
                None => output.push('-'),
            }

            output.push('\n');

            output.push_str(&format!("suggestions: {}\n", diagnostic.suggestions.len(),));

            for (suggestion_index, suggestion) in diagnostic.suggestions.iter().enumerate() {
                output.push_str(&format!(
                    "  suggestion[{}]: {}\n",
                    suggestion_index + 1,
                    escape_scalar(&suggestion.title),
                ));

                output.push_str(&format!(
                    "    applicability: {}\n",
                    suggestion.applicability.as_str(),
                ));

                output.push_str(&format!(
                    "    documentation: {}\n",
                    suggestion.documentation.len(),
                ));

                output.push_str(&format!("    edits: {}\n", suggestion.edits.len(),));

                output.push_str(&format!("    commands: {}\n", suggestion.commands.len(),));
            }
        }

        output
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

fn label_kind_token(kind: LabelKind) -> &'static str {
    match kind {
        LabelKind::Primary => "primary",
        LabelKind::Secondary => "secondary",
    }
}

fn escape_scalar(value: &str) -> String {
    let mut output = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            other => output.push(other),
        }
    }

    output
}
