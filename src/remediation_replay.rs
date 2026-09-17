//! Read-only forensic replay for history-bound remediation evidence.
//!
//! Replay reconstructs observed diagnostic lifecycle evidence. It never
//! reapplies historical edits and never claims causal proof from temporal
//! association alone.

use crate::{
    ArtifactDigest, DiagnosticHistory, DiagnosticHistoryRun, RemediationEvidenceError,
    RemediationEvidenceRecord,
};
use serde::Serialize;
use std::{error::Error, fmt};

pub const DIAGNOSTIC_REMEDIATION_REPLAY_V1_SCHEMA: &str =
    "diagprint.forensics.remediation-replay/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticRemediationState {
    Introduced,
    Resolved,
    Persisting,
    Changed,
    Absent,
}

impl DiagnosticRemediationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Introduced => "introduced",
            Self::Resolved => "resolved",
            Self::Persisting => "persisting",
            Self::Changed => "changed",
            Self::Absent => "absent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DiagnosticRemediationAssessment {
    pub evidence_record_verified: bool,
    pub post_apply_verification: bool,
    pub observed_resolution: bool,
    pub later_reappearance_observed: bool,
    pub regression_after_verified_remediation: bool,

    pub remediation_caused_resolution_established: bool,
    pub recurrence_root_cause_established: bool,
    pub git_causation_established: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticRemediationReplayStep {
    pub before_run: usize,
    pub after_run: usize,

    pub remediation_status: String,
    pub verification_checks: usize,

    pub receipt_digest: ArtifactDigest,
    pub plan_descriptor_digest: ArtifactDigest,
    pub evidence_record_digest: ArtifactDigest,

    pub before_instances: usize,
    pub after_instances: usize,

    pub observed_transition: DiagnosticRemediationState,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub later_reappearance_run: Option<usize>,

    pub assessment: DiagnosticRemediationAssessment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticRemediationReplay {
    pub schema: String,
    pub fingerprint: String,

    pub history_runs: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_chain_head: Option<ArtifactDigest>,

    pub history_chain_verified: bool,
    pub remediation_records_verified: bool,

    pub remediation_records: usize,
    pub verified_regressions: usize,

    pub steps: Vec<DiagnosticRemediationReplayStep>,
}

impl DiagnosticRemediationReplay {
    pub fn has_verified_regression(&self) -> bool {
        self.verified_regressions != 0
    }
}

impl DiagnosticHistory {
    /// Replays verified remediation evidence for one exact canonical
    /// diagnostic fingerprint.
    ///
    /// This operation is read-only. Historical fix plans are never executed.
    pub fn remediation_replay(
        &self,
        fingerprint: &str,
    ) -> Result<DiagnosticRemediationReplay, RemediationReplayError> {
        if !self
            .fingerprints()
            .iter()
            .any(|candidate| candidate == fingerprint)
        {
            return Err(RemediationReplayError::FingerprintNotFound {
                fingerprint: fingerprint.to_owned(),
            });
        }

        self.verify_remediation_evidence()?;

        let evidence = self.remediation_evidence_records()?;
        let mut steps = Vec::with_capacity(evidence.len());

        for record in &evidence {
            steps.push(build_step(self, fingerprint, record));
        }

        let verified_regressions = steps
            .iter()
            .filter(|step| step.assessment.regression_after_verified_remediation)
            .count();

        Ok(DiagnosticRemediationReplay {
            schema: DIAGNOSTIC_REMEDIATION_REPLAY_V1_SCHEMA.to_owned(),
            fingerprint: fingerprint.to_owned(),

            history_runs: self.len(),
            history_chain_head: self.head_digest(),

            history_chain_verified: true,
            remediation_records_verified: true,

            remediation_records: evidence.len(),
            verified_regressions,

            steps,
        })
    }
}

fn build_step(
    history: &DiagnosticHistory,
    fingerprint: &str,
    record: &RemediationEvidenceRecord,
) -> DiagnosticRemediationReplayStep {
    let before = &history.runs()[record.before_run];
    let after = &history.runs()[record.after_run];

    let before_digests = observation_digests(before, fingerprint);
    let after_digests = observation_digests(after, fingerprint);

    let before_instances = before_digests.len();
    let after_instances = after_digests.len();

    let observed_transition = classify_transition(&before_digests, &after_digests);

    let later_reappearance_run =
        if matches!(observed_transition, DiagnosticRemediationState::Resolved) {
            first_later_observation(history, fingerprint, record.after_run.saturating_add(1))
        } else {
            None
        };

    let post_apply_verification = record.remediation_status == "verified";

    let assessment = DiagnosticRemediationAssessment {
        evidence_record_verified: true,
        post_apply_verification,
        observed_resolution: matches!(observed_transition, DiagnosticRemediationState::Resolved),
        later_reappearance_observed: later_reappearance_run.is_some(),
        regression_after_verified_remediation: post_apply_verification
            && matches!(observed_transition, DiagnosticRemediationState::Resolved)
            && later_reappearance_run.is_some(),

        remediation_caused_resolution_established: false,
        recurrence_root_cause_established: false,
        git_causation_established: false,
    };

    DiagnosticRemediationReplayStep {
        before_run: record.before_run,
        after_run: record.after_run,

        remediation_status: record.remediation_status.clone(),
        verification_checks: record.verification_checks,

        receipt_digest: record.receipt_digest,
        plan_descriptor_digest: record.plan_descriptor_digest,
        evidence_record_digest: record.record_digest,

        before_instances,
        after_instances,

        observed_transition,
        later_reappearance_run,

        assessment,
    }
}

fn observation_digests(run: &DiagnosticHistoryRun, fingerprint: &str) -> Vec<String> {
    let mut values = run
        .observations
        .iter()
        .filter(|observation| observation.fingerprint == fingerprint)
        .map(|observation| observation.digest.clone())
        .collect::<Vec<_>>();

    values.sort();
    values
}

fn classify_transition(before: &[String], after: &[String]) -> DiagnosticRemediationState {
    match (before.is_empty(), after.is_empty()) {
        (true, true) => DiagnosticRemediationState::Absent,
        (true, false) => DiagnosticRemediationState::Introduced,
        (false, true) => DiagnosticRemediationState::Resolved,
        (false, false) if before == after => DiagnosticRemediationState::Persisting,
        (false, false) => DiagnosticRemediationState::Changed,
    }
}

fn first_later_observation(
    history: &DiagnosticHistory,
    fingerprint: &str,
    start_run: usize,
) -> Option<usize> {
    history
        .runs()
        .iter()
        .skip(start_run)
        .find(|run| {
            run.observations
                .iter()
                .any(|observation| observation.fingerprint == fingerprint)
        })
        .map(|run| run.index)
}

#[derive(Debug)]
pub enum RemediationReplayError {
    Evidence(Box<RemediationEvidenceError>),

    FingerprintNotFound { fingerprint: String },
}

impl fmt::Display for RemediationReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(source) => write!(
                formatter,
                "remediation evidence verification failed: {source}"
            ),

            Self::FingerprintNotFound { fingerprint } => write!(
                formatter,
                "diagnostic fingerprint {fingerprint:?} is not present in this history"
            ),
        }
    }
}

impl Error for RemediationReplayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Evidence(source) => Some(source.as_ref()),
            Self::FingerprintNotFound { .. } => None,
        }
    }
}

impl From<RemediationEvidenceError> for RemediationReplayError {
    fn from(source: RemediationEvidenceError) -> Self {
        Self::Evidence(Box::new(source))
    }
}
