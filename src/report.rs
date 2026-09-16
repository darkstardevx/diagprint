use crate::{Diagnostic, Severity};

/// Counts diagnostics by severity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeverityCounts {
    pub trace: usize,
    pub debug: usize,
    pub info: usize,
    pub warning: usize,
    pub error: usize,
    pub fatal: usize,
}

impl SeverityCounts {
    /// Total number of diagnostics.
    pub const fn total(self) -> usize {
        self.trace + self.debug + self.info + self.warning + self.error + self.fatal
    }

    /// Number of error and fatal diagnostics.
    pub const fn failures(self) -> usize {
        self.error + self.fatal
    }
}

/// Result of evaluating a diagnostic report against a failure threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportStatus {
    Success,
    Failure,
}

impl ReportStatus {
    /// Conventional process exit code for this report status.
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
        }
    }

    pub const fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }

    pub const fn is_failure(self) -> bool {
        matches!(self, Self::Failure)
    }
}

/// An owned collection of structured diagnostics.
///
/// `DiagnosticReport` provides common aggregation, filtering, counting, and
/// deterministic ordering without coupling the diagnostic model to a specific
/// output destination.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticReport {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    /// Creates an empty report.
    pub const fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    /// Creates an empty report with reserved capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            diagnostics: Vec::with_capacity(capacity),
        }
    }

    /// Creates a report containing one diagnostic.
    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        Self {
            diagnostics: vec![diagnostic],
        }
    }

    /// Adds a diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) -> &mut Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// Adds multiple diagnostics.
    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) -> &mut Self {
        self.diagnostics.extend(diagnostics);
        self
    }

    /// Number of diagnostics in the report.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Whether the report is empty.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Iterates over diagnostics.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    /// Mutably iterates over diagnostics.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut Diagnostic> {
        self.diagnostics.iter_mut()
    }

    /// Returns the diagnostics as a slice.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consumes the report and returns its diagnostics.
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Highest severity present in the report.
    pub fn highest_severity(&self) -> Option<Severity> {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.severity)
            .max()
    }

    /// Whether the report contains Error or Fatal diagnostics.
    pub fn has_errors(&self) -> bool {
        self.contains_at_least(Severity::Error)
    }

    /// Whether the report contains a Warning.
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Warning)
    }

    /// Whether at least one diagnostic meets or exceeds `minimum`.
    pub fn contains_at_least(&self, minimum: Severity) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity >= minimum)
    }

    /// Number of diagnostics meeting or exceeding `minimum`.
    pub fn count_at_or_above(&self, minimum: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity >= minimum)
            .count()
    }

    /// Evaluates the report using Error as the default failure threshold.
    pub fn status(&self) -> ReportStatus {
        self.status_at(Severity::Error)
    }

    /// Evaluates the report against an explicit failure threshold.
    ///
    /// This supports applications where warnings should fail CI while keeping
    /// diagprint's default behavior at Error/Fatal.
    pub fn status_at(&self, failure_threshold: Severity) -> ReportStatus {
        if self.contains_at_least(failure_threshold) {
            ReportStatus::Failure
        } else {
            ReportStatus::Success
        }
    }

    /// Conventional process exit code using Error as the failure threshold.
    pub fn exit_code(&self) -> u8 {
        self.status().exit_code()
    }

    /// Conventional process exit code for an explicit failure threshold.
    pub fn exit_code_at(&self, failure_threshold: Severity) -> u8 {
        self.status_at(failure_threshold).exit_code()
    }

    /// Counts diagnostics by severity.
    pub fn counts(&self) -> SeverityCounts {
        let mut counts = SeverityCounts::default();

        for diagnostic in &self.diagnostics {
            match diagnostic.severity {
                Severity::Trace => {
                    counts.trace += 1;
                }

                Severity::Debug => {
                    counts.debug += 1;
                }

                Severity::Info => {
                    counts.info += 1;
                }

                Severity::Warning => {
                    counts.warning += 1;
                }

                Severity::Error => {
                    counts.error += 1;
                }

                Severity::Fatal => {
                    counts.fatal += 1;
                }
            }
        }

        counts
    }

    /// Retains diagnostics at or above the supplied severity.
    pub fn retain_min_severity(&mut self, minimum: Severity) -> &mut Self {
        self.diagnostics
            .retain(|diagnostic| diagnostic.severity >= minimum);

        self
    }

    /// Returns an owned report filtered to diagnostics at or above `minimum`.
    pub fn filtered_min_severity(&self, minimum: Severity) -> Self {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity >= minimum)
            .cloned()
            .collect()
    }

    /// Sorts diagnostics into a stable, renderer-independent order.
    ///
    /// Higher severities appear first, followed by code, message, and source
    /// location. Random report IDs and timestamps are deliberately excluded.
    pub fn sort_deterministic(&mut self) -> &mut Self {
        self.diagnostics.sort_by(|left, right| {
            right
                .severity
                .cmp(&left.severity)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
                .then_with(|| {
                    let left_location = left.labels.first().map(|label| {
                        (
                            label.location.file.as_str(),
                            label.location.line,
                            label.location.column,
                        )
                    });

                    let right_location = right.labels.first().map(|label| {
                        (
                            label.location.file.as_str(),
                            label.location.line,
                            label.location.column,
                        )
                    });

                    left_location.cmp(&right_location)
                })
        });

        self
    }
}

impl From<Diagnostic> for DiagnosticReport {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::from_diagnostic(diagnostic)
    }
}

impl FromIterator<Diagnostic> for DiagnosticReport {
    fn from_iter<T: IntoIterator<Item = Diagnostic>>(iter: T) -> Self {
        Self {
            diagnostics: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for DiagnosticReport {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}

impl<'a> IntoIterator for &'a DiagnosticReport {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.iter()
    }
}
