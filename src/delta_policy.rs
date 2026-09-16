use crate::{DeltaKind, DiagnosticDelta, ReportStatus, Severity};

/// One CI/evaluation rule applied to a [`DiagnosticDelta`].
///
/// Rules describe policy only. Diagnostic matching and classification are
/// performed by [`DiagnosticDelta`] before policy evaluation begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaRule {
    /// Fail when the baseline and candidate differ in any way.
    AnyDifference,

    /// Fail when a brand-new diagnostic reaches `minimum`.
    ///
    /// This does not include an existing diagnostic whose severity crosses
    /// the threshold. Use [`Self::SeverityIncreaseAtOrAbove`] or
    /// [`Self::IntroducedAtOrAbove`] for that behavior.
    NewAtOrAbove { minimum: Severity },

    /// Fail when the number of diagnostics at or above `minimum` increases
    /// within a logical fingerprint bucket.
    ///
    /// This is multiset-aware and includes both genuinely new diagnostics and
    /// existing diagnostics promoted across the threshold.
    IntroducedAtOrAbove { minimum: Severity },

    /// Fail when an existing logical diagnostic increases in severity and its
    /// candidate severity reaches at least `minimum`.
    SeverityIncreaseAtOrAbove { minimum: Severity },
}

impl DeltaRule {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AnyDifference => "any_difference",
            Self::NewAtOrAbove { .. } => "new_at_or_above",
            Self::IntroducedAtOrAbove { .. } => "introduced_at_or_above",
            Self::SeverityIncreaseAtOrAbove { .. } => "severity_increase_at_or_above",
        }
    }

    pub const fn minimum_severity(self) -> Option<Severity> {
        match self {
            Self::AnyDifference => None,

            Self::NewAtOrAbove { minimum }
            | Self::IntroducedAtOrAbove { minimum }
            | Self::SeverityIncreaseAtOrAbove { minimum } => Some(minimum),
        }
    }
}

/// One failed [`DeltaRule`] and the number of matching diagnostic instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeltaViolation {
    rule: DeltaRule,
    count: usize,
}

impl DeltaViolation {
    const fn new(rule: DeltaRule, count: usize) -> Self {
        Self { rule, count }
    }

    pub const fn rule(self) -> DeltaRule {
        self.rule
    }

    pub const fn count(self) -> usize {
        self.count
    }
}

/// Result of evaluating a [`DiagnosticDelta`] against a [`DeltaPolicy`].
///
/// A successful evaluation contains no violations. A failed evaluation keeps
/// each failed rule separately so CI renderers and exporters can explain why a
/// baseline comparison failed without recomputing policy logic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeltaEvaluation {
    violations: Vec<DeltaViolation>,
}

impl DeltaEvaluation {
    fn new(violations: Vec<DeltaViolation>) -> Self {
        Self { violations }
    }

    pub fn violations(&self) -> &[DeltaViolation] {
        &self.violations
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &DeltaViolation> {
        self.violations.iter()
    }

    pub fn len(&self) -> usize {
        self.violations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn status(&self) -> ReportStatus {
        if self.violations.is_empty() {
            ReportStatus::Success
        } else {
            ReportStatus::Failure
        }
    }

    pub fn is_success(&self) -> bool {
        self.status().is_success()
    }

    pub fn is_failure(&self) -> bool {
        self.status().is_failure()
    }

    /// Conventional process exit code suitable for CI.
    pub fn exit_code(&self) -> u8 {
        self.status().exit_code()
    }

    /// Returns the violation for `rule`, if that rule failed.
    pub fn violation(&self, rule: DeltaRule) -> Option<&DeltaViolation> {
        self.violations
            .iter()
            .find(|violation| violation.rule == rule)
    }
}

/// Baseline-aware policy for evaluating a [`DiagnosticDelta`].
///
/// `DeltaPolicy::new()` is permissive and contains no failure rules.
///
/// [`DeltaPolicy::ci()`] provides the recommended baseline-aware CI preset:
///
/// - brand-new Error/Fatal diagnostics fail;
/// - severity increases which reach Error/Fatal fail.
///
/// Existing Error/Fatal diagnostics do not fail merely because non-severity
/// content changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeltaPolicy {
    any_difference: bool,
    new_at_or_above: Option<Severity>,
    introduced_at_or_above: Option<Severity>,
    severity_increase_at_or_above: Option<Severity>,
}

impl Default for DeltaPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl DeltaPolicy {
    /// Creates a policy with no failure rules.
    pub const fn new() -> Self {
        Self {
            any_difference: false,
            new_at_or_above: None,
            introduced_at_or_above: None,
            severity_increase_at_or_above: None,
        }
    }

    /// Baseline-aware CI preset.
    ///
    /// This rejects:
    ///
    /// - new Error/Fatal diagnostics;
    /// - Warning -> Error/Fatal promotions;
    /// - Error -> Fatal promotions.
    ///
    /// It does not reject an existing Error simply because its help, notes,
    /// labels, remediation, or other digest-bearing content changed.
    pub const fn ci() -> Self {
        Self::new()
            .fail_on_new_at_or_above(Severity::Error)
            .fail_on_severity_increase_at_or_above(Severity::Error)
    }

    pub const fn fail_on_any_difference(mut self) -> Self {
        self.any_difference = true;
        self
    }

    pub const fn fail_on_new_at_or_above(mut self, minimum: Severity) -> Self {
        self.new_at_or_above = Some(minimum);
        self
    }

    pub const fn fail_on_introduced_at_or_above(mut self, minimum: Severity) -> Self {
        self.introduced_at_or_above = Some(minimum);
        self
    }

    pub const fn fail_on_severity_increase_at_or_above(mut self, minimum: Severity) -> Self {
        self.severity_increase_at_or_above = Some(minimum);
        self
    }

    /// Convenience rule for brand-new Error/Fatal diagnostics.
    pub const fn fail_on_new_errors(self) -> Self {
        self.fail_on_new_at_or_above(Severity::Error)
    }

    /// Convenience rule for brand-new Warning/Error/Fatal diagnostics.
    pub const fn fail_on_new_warnings_or_worse(self) -> Self {
        self.fail_on_new_at_or_above(Severity::Warning)
    }

    /// Convenience rule for increases which reach Error/Fatal severity.
    pub const fn fail_on_error_regressions(self) -> Self {
        self.fail_on_severity_increase_at_or_above(Severity::Error)
    }

    pub const fn fails_on_any_difference(&self) -> bool {
        self.any_difference
    }

    pub const fn new_threshold(&self) -> Option<Severity> {
        self.new_at_or_above
    }

    pub const fn introduced_threshold(&self) -> Option<Severity> {
        self.introduced_at_or_above
    }

    pub const fn severity_increase_threshold(&self) -> Option<Severity> {
        self.severity_increase_at_or_above
    }

    pub const fn is_permissive(&self) -> bool {
        !self.any_difference
            && self.new_at_or_above.is_none()
            && self.introduced_at_or_above.is_none()
            && self.severity_increase_at_or_above.is_none()
    }

    /// Evaluates a previously computed diagnostic delta.
    pub fn evaluate(&self, delta: &DiagnosticDelta) -> DeltaEvaluation {
        let mut violations = Vec::new();

        if self.any_difference {
            push_violation(
                &mut violations,
                DeltaRule::AnyDifference,
                delta.counts().differences(),
            );
        }

        if let Some(minimum) = self.new_at_or_above {
            push_violation(
                &mut violations,
                DeltaRule::NewAtOrAbove { minimum },
                count_new_at_or_above(delta, minimum),
            );
        }

        if let Some(minimum) = self.introduced_at_or_above {
            push_violation(
                &mut violations,
                DeltaRule::IntroducedAtOrAbove { minimum },
                delta.introduced_at_or_above(minimum),
            );
        }

        if let Some(minimum) = self.severity_increase_at_or_above {
            push_violation(
                &mut violations,
                DeltaRule::SeverityIncreaseAtOrAbove { minimum },
                delta.severity_increases_at_or_above(minimum),
            );
        }

        DeltaEvaluation::new(violations)
    }
}

impl DiagnosticDelta {
    /// Evaluates this delta against `policy`.
    pub fn evaluate(&self, policy: &DeltaPolicy) -> DeltaEvaluation {
        policy.evaluate(self)
    }
}

fn push_violation(violations: &mut Vec<DeltaViolation>, rule: DeltaRule, count: usize) {
    if count != 0 {
        violations.push(DeltaViolation::new(rule, count));
    }
}

fn count_new_at_or_above(delta: &DiagnosticDelta, minimum: Severity) -> usize {
    delta
        .iter_kind(DeltaKind::New)
        .filter(|entry| {
            entry
                .candidate()
                .is_some_and(|diagnostic| diagnostic.severity >= minimum)
        })
        .count()
}
