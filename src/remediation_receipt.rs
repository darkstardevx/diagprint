use crate::{
    Applicability, ArtifactDigest, CanonicalizationError, DiagnosticReport, FixPlan, FixPlanReport,
    ReportDigest, Severity,
};
use serde::Serialize;
use std::{error::Error, fmt};

/// Stable schema identifier for remediation receipts.
pub const REMEDIATION_RECEIPT_V1_SCHEMA: &str = "diagprint.remediation.receipt/v1";

/// Successful remediation state.
///
/// `Applied` means the transaction completed successfully but the plan did not
/// define post-apply verification checks.
///
/// `Verified` means the transaction completed successfully and every planned
/// post-apply verification check passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationStatus {
    Applied,
    Verified,
}

impl RemediationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Verified => "verified",
        }
    }

    pub const fn is_verified(self) -> bool {
        matches!(self, Self::Verified)
    }
}

/// Privacy-safe identity and summary of the exact fix plan used.
///
/// The descriptor itself is deliberately not embedded here because it may
/// contain source paths, expected source contents, replacement contents, and
/// verification values.
#[derive(Debug, Clone, Serialize)]
pub struct RemediationPlanReceipt {
    pub descriptor_schema: &'static str,

    pub descriptor_digest: ArtifactDigest,
    pub descriptor_byte_length: usize,

    pub title: String,
    pub applicability: Applicability,

    pub preconditions: usize,
    pub edits: usize,
    pub verifications: usize,

    pub backups_enabled: bool,
}

/// Result of the successful filesystem remediation transaction.
///
/// Changed file paths are deliberately omitted from this receipt. Their count
/// is retained while their exact identities remain bound by the fix-plan
/// descriptor digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RemediationOutcome {
    pub status: RemediationStatus,

    pub changed_files: usize,

    pub verification_checks: usize,
    pub verification_passed: bool,
}

/// Semantic diagnostic effect observed after remediation and rescanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RemediationEffect {
    pub before_diagnostics: usize,
    pub after_diagnostics: usize,

    pub new: usize,
    pub resolved: usize,
    pub persisting: usize,
    pub changed: usize,

    pub introduced_errors: usize,
    pub severity_increases: usize,
}

impl RemediationEffect {
    /// Number of diagnostic entries whose semantic state differs between the
    /// before and after reports.
    pub const fn differences(self) -> usize {
        self.new + self.resolved + self.changed
    }

    pub const fn is_unchanged(self) -> bool {
        self.differences() == 0
    }
}

/// Evidence tying one exact remediation plan to before/after diagnostic state.
///
/// A remediation receipt intentionally contains no source text, replacement
/// text, expected edit contents, changed file paths, or verification payloads.
///
/// Those details are bound through `plan.descriptor_digest` and can later be
/// included separately under a diagnostic-capsule export policy.
#[derive(Debug, Clone, Serialize)]
pub struct RemediationReceipt {
    pub schema: &'static str,

    pub before_report: ReportDigest,
    pub after_report: ReportDigest,

    pub plan: RemediationPlanReceipt,

    pub outcome: RemediationOutcome,

    pub effect: RemediationEffect,
}

impl RemediationReceipt {
    /// Creates a receipt for a successfully applied [`FixPlan`].
    ///
    /// The caller supplies both the diagnostic report captured before the
    /// remediation and the report produced after rescanning the mutated
    /// project.
    ///
    /// This function does not execute the plan and does not perform a scan.
    pub fn from_successful_apply(
        before: &DiagnosticReport,
        after: &DiagnosticReport,
        plan: &FixPlan,
        applied: &FixPlanReport,
    ) -> Result<Self, RemediationReceiptError> {
        if !applied.verification_passed {
            return Err(RemediationReceiptError::VerificationNotPassed);
        }

        let descriptor = plan.descriptor();

        let expected_verifications = descriptor.verifications.len();

        if applied.verification_checks != expected_verifications {
            return Err(RemediationReceiptError::VerificationCountMismatch {
                expected: expected_verifications,
                actual: applied.verification_checks,
            });
        }

        let descriptor_bytes = serde_json::to_vec(&descriptor)
            .map_err(RemediationReceiptError::DescriptorSerialization)?;

        let delta = after
            .delta_from(before)
            .map_err(RemediationReceiptError::Canonicalization)?;

        let counts = delta.counts();

        let status = if applied.verification_checks == 0 {
            RemediationStatus::Applied
        } else {
            RemediationStatus::Verified
        };

        Ok(Self {
            schema: REMEDIATION_RECEIPT_V1_SCHEMA,

            before_report: delta.baseline_digest(),
            after_report: delta.candidate_digest(),

            plan: RemediationPlanReceipt {
                descriptor_schema: descriptor.schema,

                descriptor_digest: ArtifactDigest::compute(&descriptor_bytes),

                descriptor_byte_length: descriptor_bytes.len(),

                title: descriptor.title,

                applicability: descriptor.applicability,

                preconditions: descriptor.preconditions.len(),

                edits: descriptor.edits.len(),

                verifications: descriptor.verifications.len(),

                backups_enabled: descriptor.backups,
            },

            outcome: RemediationOutcome {
                status,

                changed_files: applied.changed_files.len(),

                verification_checks: applied.verification_checks,

                verification_passed: applied.verification_passed,
            },

            effect: RemediationEffect {
                before_diagnostics: counts.baseline_total(),

                after_diagnostics: counts.candidate_total(),

                new: counts.new,

                resolved: counts.resolved,

                persisting: counts.persisting,

                changed: counts.changed,

                introduced_errors: delta.introduced_at_or_above(Severity::Error),

                severity_increases: delta.severity_increases(),
            },
        })
    }

    pub const fn status(&self) -> RemediationStatus {
        self.outcome.status
    }

    pub const fn is_verified(&self) -> bool {
        self.outcome.status.is_verified()
    }

    /// Whether the receipt records any diagnostic-state difference between the
    /// before and after reports.
    pub const fn changed_diagnostic_state(&self) -> bool {
        !self.effect.is_unchanged()
    }
}

/// Failure while constructing a remediation receipt.
#[derive(Debug)]
pub enum RemediationReceiptError {
    Canonicalization(CanonicalizationError),

    DescriptorSerialization(serde_json::Error),

    VerificationNotPassed,

    VerificationCountMismatch { expected: usize, actual: usize },
}

impl fmt::Display for RemediationReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonicalization(error) => {
                write!(
                    formatter,
                    "could not compute remediation report identity: {error}"
                )
            }

            Self::DescriptorSerialization(error) => {
                write!(
                    formatter,
                    "could not serialize fix-plan descriptor: {error}"
                )
            }

            Self::VerificationNotPassed => {
                formatter.write_str(
                    "successful remediation receipt requires a FixPlanReport with verification_passed=true",
                )
            }

            Self::VerificationCountMismatch {
                expected,
                actual,
            } => write!(
                formatter,
                "fix-plan verification count mismatch: plan contains {expected}, apply report records {actual}",
            ),
        }
    }
}

impl Error for RemediationReceiptError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonicalization(error) => Some(error),

            Self::DescriptorSerialization(error) => Some(error),

            Self::VerificationNotPassed | Self::VerificationCountMismatch { .. } => None,
        }
    }
}
