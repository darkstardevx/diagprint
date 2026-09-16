use crate::{
    DeltaArtifact, DeltaArtifactEntry, DeltaArtifactEvaluation, DeltaArtifactViolation,
    ExportDiagnostic, ExportLabel, LabelKind, Severity,
};

/// Presents `diagprint.delta/v1` artifacts as GitHub Actions workflow commands.
///
/// The renderer consumes only [`DeltaArtifact`]. Diagnostic data has therefore
/// already crossed the crate's [`crate::ExportPolicy`] boundary before GitHub
/// presentation occurs.
///
/// By default:
///
/// - one delta summary is emitted;
/// - new diagnostics are annotated;
/// - changed diagnostics are annotated;
/// - resolved diagnostics are omitted;
/// - persisting diagnostics are omitted.
///
/// Resolved and persisting annotations can be enabled explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GithubActionsDeltaRenderer {
    include_summary: bool,
    include_resolved: bool,
    include_persisting: bool,
}

impl Default for GithubActionsDeltaRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubActionsDeltaRenderer {
    pub const fn new() -> Self {
        Self {
            include_summary: true,
            include_resolved: false,
            include_persisting: false,
        }
    }

    pub const fn with_summary(mut self, enabled: bool) -> Self {
        self.include_summary = enabled;
        self
    }

    pub const fn with_resolved(mut self, enabled: bool) -> Self {
        self.include_resolved = enabled;
        self
    }

    pub const fn with_persisting(mut self, enabled: bool) -> Self {
        self.include_persisting = enabled;
        self
    }

    pub const fn includes_summary(&self) -> bool {
        self.include_summary
    }

    pub const fn includes_resolved(&self) -> bool {
        self.include_resolved
    }

    pub const fn includes_persisting(&self) -> bool {
        self.include_persisting
    }

    /// Renders a portable delta artifact as GitHub Actions workflow commands.
    pub fn render(&self, artifact: &DeltaArtifact) -> String {
        let mut output = Vec::new();

        if self.include_summary {
            output.push(Self::render_summary(artifact));
        }

        if let Some(evaluation) = &artifact.evaluation {
            output.extend(Self::render_evaluation(evaluation));
        }

        for entry in &artifact.entries {
            if let Some(annotation) = self.render_entry(entry) {
                output.push(annotation);
            }
        }

        output.join("\n")
    }

    fn render_summary(artifact: &DeltaArtifact) -> String {
        let command = artifact
            .evaluation
            .as_ref()
            .map(|evaluation| {
                if evaluation.exit_code == 0 {
                    "notice"
                } else {
                    "error"
                }
            })
            .unwrap_or("notice");

        let counts = artifact.counts;

        let message = format!(
            "new={}, changed={}, resolved={}, persisting={}, baseline_total={}, candidate_total={}",
            counts.new,
            counts.changed,
            counts.resolved,
            counts.persisting,
            counts.baseline_total,
            counts.candidate_total,
        );

        Self::workflow_command(command, &[("title", "diagprint delta")], &message)
    }

    fn render_evaluation(evaluation: &DeltaArtifactEvaluation) -> Vec<String> {
        if evaluation.violations.is_empty() {
            return vec![Self::workflow_command(
                "notice",
                &[("title", "diagprint CI policy")],
                "baseline-aware diagnostic policy passed",
            )];
        }

        evaluation
            .violations
            .iter()
            .map(Self::render_violation)
            .collect()
    }

    fn render_violation(violation: &DeltaArtifactViolation) -> String {
        let message = match violation.minimum_severity {
            Some(minimum) => format!(
                "{} matched {} diagnostic instance(s) at or above {}",
                violation.rule,
                violation.count,
                minimum.as_str(),
            ),

            None => format!(
                "{} matched {} diagnostic difference(s)",
                violation.rule, violation.count,
            ),
        };

        Self::workflow_command(
            "error",
            &[("title", "diagprint CI policy violation")],
            &message,
        )
    }

    fn render_entry(&self, entry: &DeltaArtifactEntry) -> Option<String> {
        match entry.kind {
            "new" => {
                let diagnostic = entry.candidate.as_ref()?;

                Some(Self::render_diagnostic_entry(
                    "new",
                    diagnostic,
                    Self::command(diagnostic.severity),
                    None,
                ))
            }

            "changed" => {
                let candidate = entry.candidate.as_ref()?;

                let transition = entry
                    .baseline
                    .as_ref()
                    .map(|baseline| (baseline.severity, candidate.severity));

                Some(Self::render_diagnostic_entry(
                    "changed",
                    candidate,
                    Self::command(candidate.severity),
                    transition,
                ))
            }

            "resolved" if self.include_resolved => {
                let diagnostic = entry.baseline.as_ref()?;

                Some(Self::render_diagnostic_entry(
                    "resolved", diagnostic, "notice", None,
                ))
            }

            "persisting" if self.include_persisting => {
                let diagnostic = entry.candidate.as_ref()?;

                Some(Self::render_diagnostic_entry(
                    "persisting",
                    diagnostic,
                    "notice",
                    None,
                ))
            }

            _ => None,
        }
    }

    fn render_diagnostic_entry(
        kind: &str,
        diagnostic: &ExportDiagnostic,
        command: &'static str,
        transition: Option<(Severity, Severity)>,
    ) -> String {
        let label = Self::primary_label(diagnostic);

        let mut properties = Vec::new();

        let title = match &diagnostic.code {
            Some(code) => format!("[{kind}] {code}"),
            None => format!("[{kind}] diagnostic"),
        };

        properties.push(("title".to_owned(), title));

        if let Some(label) = label {
            if let Some(file) = &label.location.file {
                properties.push(("file".to_owned(), file.clone()));
                properties.push(("line".to_owned(), label.location.line.to_string()));

                if let Some(column) = label.location.column {
                    properties.push(("col".to_owned(), column.to_string()));

                    if let Some(length) = label.length {
                        if length > 0 {
                            let length = u32::try_from(length).unwrap_or(u32::MAX);

                            let end_column = column.saturating_add(length.saturating_sub(1));

                            properties.push(("endColumn".to_owned(), end_column.to_string()));
                        }
                    }
                }
            }
        }

        let mut message = diagnostic.message.clone();

        if let Some(label_message) = label.and_then(|label| label.message.as_deref()) {
            message.push('\n');
            message.push_str(label_message);
        }

        if let Some((baseline, candidate)) = transition {
            if baseline != candidate {
                message.push_str("\nseverity: ");
                message.push_str(baseline.as_str());
                message.push_str(" -> ");
                message.push_str(candidate.as_str());
            } else {
                message.push_str("\ncontent changed");
            }
        }

        let properties = properties
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();

        Self::workflow_command(command, &properties, &message)
    }

    fn primary_label(diagnostic: &ExportDiagnostic) -> Option<&ExportLabel> {
        diagnostic
            .labels
            .iter()
            .find(|label| label.kind == LabelKind::Primary)
            .or_else(|| diagnostic.labels.first())
    }

    const fn command(severity: Severity) -> &'static str {
        match severity {
            Severity::Trace | Severity::Debug | Severity::Info => "notice",
            Severity::Warning => "warning",
            Severity::Error | Severity::Fatal => "error",
        }
    }

    fn workflow_command(command: &str, properties: &[(&str, &str)], message: &str) -> String {
        let properties = properties
            .iter()
            .map(|(name, value)| format!("{name}={}", Self::escape_property(value),))
            .collect::<Vec<_>>();

        let message = Self::escape_data(message);

        if properties.is_empty() {
            format!("::{command}::{message}")
        } else {
            format!("::{command} {}::{message}", properties.join(","),)
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
}
