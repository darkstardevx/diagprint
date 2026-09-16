use crate::{
    DeltaCounts, DeltaEvaluation, DeltaPolicy, DeltaViolation, DiagnosticChange, DiagnosticDelta,
    DiagnosticDigest, DiagnosticFingerprint, ExportDiagnostic, ExportPolicy, FingerprintPolicy,
    FingerprintSource, ReportDigest, Severity,
};
use serde::Serialize;

/// Stable schema identifier for portable diagnostic delta artifacts.
pub const DELTA_V1_SCHEMA: &str = "diagprint.delta/v1";

/// Portable, privacy-aware representation of a semantic diagnostic delta.
///
/// The artifact contains canonical report identity, the fingerprint policy used
/// for matching, deterministic per-diagnostic classifications, and optional CI
/// evaluation metadata.
///
/// Diagnostic payloads cross the external boundary only through
/// [`ExportDiagnostic`] and therefore obey an explicit [`ExportPolicy`].
#[derive(Debug, Clone, Serialize)]
pub struct DeltaArtifact {
    pub schema: &'static str,

    pub baseline_report: ReportDigest,
    pub candidate_report: ReportDigest,

    pub fingerprint_policy: DeltaArtifactFingerprintPolicy,

    pub counts: DeltaArtifactCounts,

    pub entries: Vec<DeltaArtifactEntry>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<DeltaArtifactEvaluation>,
}

/// Serializable fingerprint policy recorded in a delta artifact.
#[derive(Debug, Clone, Serialize)]
pub struct DeltaArtifactFingerprintPolicy {
    pub include_code: bool,
    pub include_message: bool,
    pub source: &'static str,
    pub include_primary_label_message: bool,
    pub prefer_explicit_identity: bool,
}

/// Serializable delta summary.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeltaArtifactCounts {
    pub new: usize,
    pub resolved: usize,
    pub persisting: usize,
    pub changed: usize,

    pub baseline_total: usize,
    pub candidate_total: usize,
    pub differences: usize,
}

/// One portable semantic delta entry.
#[derive(Debug, Clone, Serialize)]
pub struct DeltaArtifactEntry {
    pub kind: &'static str,

    pub fingerprint: DiagnosticFingerprint,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_digest: Option<DiagnosticDigest>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_digest: Option<DiagnosticDigest>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<ExportDiagnostic>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<ExportDiagnostic>,
}

/// Portable CI policy and evaluation result attached to a delta artifact.
#[derive(Debug, Clone, Serialize)]
pub struct DeltaArtifactEvaluation {
    pub status: &'static str,
    pub exit_code: u8,

    pub policy: DeltaArtifactPolicy,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<DeltaArtifactViolation>,
}

/// Serializable form of [`DeltaPolicy`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeltaArtifactPolicy {
    pub fail_on_any_difference: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_at_or_above: Option<Severity>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduced_at_or_above: Option<Severity>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity_increase_at_or_above: Option<Severity>,
}

/// One machine-readable failed CI rule.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeltaArtifactViolation {
    pub rule: &'static str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_severity: Option<Severity>,

    pub count: usize,
}

impl DeltaArtifact {
    /// Creates an unevaluated portable delta artifact.
    pub fn new(delta: &DiagnosticDelta, export_policy: &ExportPolicy) -> Self {
        Self {
            schema: DELTA_V1_SCHEMA,

            baseline_report: delta.baseline_digest(),
            candidate_report: delta.candidate_digest(),

            fingerprint_policy: DeltaArtifactFingerprintPolicy::from(delta.fingerprint_policy()),

            counts: DeltaArtifactCounts::from(delta.counts()),

            entries: delta
                .iter()
                .map(|entry| DeltaArtifactEntry::from_change(entry, export_policy))
                .collect(),

            evaluation: None,
        }
    }

    /// Creates a portable delta artifact and evaluates it against `policy`.
    ///
    /// Evaluation is performed here so the serialized policy and result cannot
    /// accidentally refer to different delta evaluations.
    pub fn evaluated(
        delta: &DiagnosticDelta,
        policy: &DeltaPolicy,
        export_policy: &ExportPolicy,
    ) -> Self {
        let evaluation = policy.evaluate(delta);

        let mut artifact = Self::new(delta, export_policy);

        artifact.evaluation = Some(DeltaArtifactEvaluation::from_policy_and_evaluation(
            policy,
            &evaluation,
        ));

        artifact
    }

    /// Serializes the artifact as compact JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Serializes the artifact as deterministic pretty JSON.
    pub fn to_json_pretty(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

impl DiagnosticDelta {
    /// Creates a privacy-aware portable delta artifact.
    pub fn artifact(&self, export_policy: &ExportPolicy) -> DeltaArtifact {
        DeltaArtifact::new(self, export_policy)
    }

    /// Creates a portable delta artifact containing a CI evaluation.
    pub fn evaluated_artifact(
        &self,
        policy: &DeltaPolicy,
        export_policy: &ExportPolicy,
    ) -> DeltaArtifact {
        DeltaArtifact::evaluated(self, policy, export_policy)
    }
}

impl DeltaArtifactEntry {
    fn from_change(change: &DiagnosticChange, export_policy: &ExportPolicy) -> Self {
        Self {
            kind: change.kind().as_str(),

            fingerprint: change.fingerprint(),

            baseline_digest: change.baseline_digest(),
            candidate_digest: change.candidate_digest(),

            baseline: change
                .baseline()
                .map(|diagnostic| ExportDiagnostic::with_policy(diagnostic, export_policy)),

            candidate: change
                .candidate()
                .map(|diagnostic| ExportDiagnostic::with_policy(diagnostic, export_policy)),
        }
    }
}

impl From<DeltaCounts> for DeltaArtifactCounts {
    fn from(counts: DeltaCounts) -> Self {
        Self {
            new: counts.new,
            resolved: counts.resolved,
            persisting: counts.persisting,
            changed: counts.changed,

            baseline_total: counts.baseline_total(),
            candidate_total: counts.candidate_total(),
            differences: counts.differences(),
        }
    }
}

impl From<&FingerprintPolicy> for DeltaArtifactFingerprintPolicy {
    fn from(policy: &FingerprintPolicy) -> Self {
        Self {
            include_code: policy.includes_code(),
            include_message: policy.includes_message(),

            source: fingerprint_source_name(policy.source()),

            include_primary_label_message: policy.includes_primary_label_message(),

            prefer_explicit_identity: policy.prefers_explicit_identity(),
        }
    }
}

impl DeltaArtifactEvaluation {
    fn from_policy_and_evaluation(policy: &DeltaPolicy, evaluation: &DeltaEvaluation) -> Self {
        Self {
            status: if evaluation.is_success() {
                "success"
            } else {
                "failure"
            },

            exit_code: evaluation.exit_code(),

            policy: DeltaArtifactPolicy::from(policy),

            violations: evaluation
                .iter()
                .copied()
                .map(DeltaArtifactViolation::from)
                .collect(),
        }
    }
}

impl From<&DeltaPolicy> for DeltaArtifactPolicy {
    fn from(policy: &DeltaPolicy) -> Self {
        Self {
            fail_on_any_difference: policy.fails_on_any_difference(),

            new_at_or_above: policy.new_threshold(),

            introduced_at_or_above: policy.introduced_threshold(),

            severity_increase_at_or_above: policy.severity_increase_threshold(),
        }
    }
}

impl From<DeltaViolation> for DeltaArtifactViolation {
    fn from(violation: DeltaViolation) -> Self {
        let rule = violation.rule();

        Self {
            rule: rule.as_str(),
            minimum_severity: rule.minimum_severity(),
            count: violation.count(),
        }
    }
}

const fn fingerprint_source_name(source: FingerprintSource) -> &'static str {
    match source {
        FingerprintSource::Omit => "omit",
        FingerprintSource::FileName => "file_name",
        FingerprintSource::Full => "full",
    }
}
