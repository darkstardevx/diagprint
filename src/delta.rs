use crate::{
    CanonicalizationError, Diagnostic, DiagnosticDigest, DiagnosticFingerprint, DiagnosticReport,
    FingerprintPolicy, ReportDigest, Severity,
};
use std::{cmp::Ordering, collections::BTreeMap};

/// Classification of one diagnostic instance between a baseline and candidate
/// report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeltaKind {
    /// Present only in the candidate report.
    New,

    /// Present only in the baseline report.
    Resolved,

    /// Present in both reports with identical canonical content.
    Persisting,

    /// Has the same logical fingerprint in both reports but different
    /// canonical diagnostic content.
    Changed,
}

impl DeltaKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Resolved => "resolved",
            Self::Persisting => "persisting",
            Self::Changed => "changed",
        }
    }
}

/// Counts diagnostic instances by delta classification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeltaCounts {
    pub new: usize,
    pub resolved: usize,
    pub persisting: usize,
    pub changed: usize,
}

impl DeltaCounts {
    /// Total number of delta entries.
    ///
    /// A `Changed` entry represents one baseline diagnostic paired with one
    /// candidate diagnostic and therefore counts as one delta entry.
    pub const fn total_entries(self) -> usize {
        self.new + self.resolved + self.persisting + self.changed
    }

    /// Number of baseline diagnostic instances represented by this delta.
    pub const fn baseline_total(self) -> usize {
        self.resolved + self.persisting + self.changed
    }

    /// Number of candidate diagnostic instances represented by this delta.
    pub const fn candidate_total(self) -> usize {
        self.new + self.persisting + self.changed
    }

    /// Number of entries representing a difference between baseline and
    /// candidate.
    pub const fn differences(self) -> usize {
        self.new + self.resolved + self.changed
    }

    pub const fn is_unchanged(self) -> bool {
        self.differences() == 0
    }
}

/// One classified diagnostic instance in a [`DiagnosticDelta`].
///
/// Variants enforce the structural invariants of each classification:
///
/// - `New` owns only candidate state;
/// - `Resolved` owns only baseline state;
/// - `Persisting` owns both states and one shared content digest;
/// - `Changed` owns both states and distinct content digests.
#[derive(Debug, Clone)]
pub enum DiagnosticChange {
    New {
        fingerprint: DiagnosticFingerprint,
        candidate_digest: DiagnosticDigest,
        candidate: Diagnostic,
    },

    Resolved {
        fingerprint: DiagnosticFingerprint,
        baseline_digest: DiagnosticDigest,
        baseline: Diagnostic,
    },

    Persisting {
        fingerprint: DiagnosticFingerprint,
        digest: DiagnosticDigest,
        baseline: Diagnostic,
        candidate: Diagnostic,
    },

    Changed {
        fingerprint: DiagnosticFingerprint,
        baseline_digest: DiagnosticDigest,
        candidate_digest: DiagnosticDigest,
        baseline: Diagnostic,
        candidate: Diagnostic,
    },
}

impl DiagnosticChange {
    pub fn kind(&self) -> DeltaKind {
        match self {
            Self::New { .. } => DeltaKind::New,
            Self::Resolved { .. } => DeltaKind::Resolved,
            Self::Persisting { .. } => DeltaKind::Persisting,
            Self::Changed { .. } => DeltaKind::Changed,
        }
    }

    pub fn fingerprint(&self) -> DiagnosticFingerprint {
        match self {
            Self::New { fingerprint, .. }
            | Self::Resolved { fingerprint, .. }
            | Self::Persisting { fingerprint, .. }
            | Self::Changed { fingerprint, .. } => *fingerprint,
        }
    }

    pub fn baseline(&self) -> Option<&Diagnostic> {
        match self {
            Self::Resolved { baseline, .. }
            | Self::Persisting { baseline, .. }
            | Self::Changed { baseline, .. } => Some(baseline),

            Self::New { .. } => None,
        }
    }

    pub fn candidate(&self) -> Option<&Diagnostic> {
        match self {
            Self::New { candidate, .. }
            | Self::Persisting { candidate, .. }
            | Self::Changed { candidate, .. } => Some(candidate),

            Self::Resolved { .. } => None,
        }
    }

    pub fn baseline_digest(&self) -> Option<DiagnosticDigest> {
        match self {
            Self::New { .. } => None,

            Self::Resolved {
                baseline_digest, ..
            }
            | Self::Changed {
                baseline_digest, ..
            } => Some(*baseline_digest),

            Self::Persisting { digest, .. } => Some(*digest),
        }
    }

    pub fn candidate_digest(&self) -> Option<DiagnosticDigest> {
        match self {
            Self::Resolved { .. } => None,

            Self::New {
                candidate_digest, ..
            }
            | Self::Changed {
                candidate_digest, ..
            } => Some(*candidate_digest),

            Self::Persisting { digest, .. } => Some(*digest),
        }
    }

    /// Returns the baseline/candidate severity pair for a changed diagnostic.
    pub fn severity_transition(&self) -> Option<(Severity, Severity)> {
        match self {
            Self::Changed {
                baseline,
                candidate,
                ..
            } => Some((baseline.severity, candidate.severity)),

            _ => None,
        }
    }

    /// Whether this entry represents a severity increase.
    pub fn is_severity_increase(&self) -> bool {
        self.severity_transition()
            .map(|(baseline, candidate)| candidate > baseline)
            .unwrap_or(false)
    }
}

/// Semantic comparison between a baseline and candidate diagnostic report.
///
/// Matching is based on [`DiagnosticFingerprint`] while change detection uses
/// [`DiagnosticDigest`].
///
/// Diagnostics are treated as a multiset rather than a set. Duplicate
/// diagnostics are therefore preserved.
///
/// Within one fingerprint bucket, exact digest matches are paired first.
/// Remaining baseline and candidate instances are paired deterministically by
/// digest and classified as [`DeltaKind::Changed`]. Remaining unpaired
/// instances become [`DeltaKind::Resolved`] or [`DeltaKind::New`].
#[derive(Debug, Clone)]
pub struct DiagnosticDelta {
    baseline_digest: ReportDigest,
    candidate_digest: ReportDigest,
    fingerprint_policy: FingerprintPolicy,
    entries: Vec<DiagnosticChange>,
    counts: DeltaCounts,
}

impl DiagnosticDelta {
    /// Compares `baseline` with `candidate` using the canonical default
    /// fingerprint policy.
    pub fn between(
        baseline: &DiagnosticReport,
        candidate: &DiagnosticReport,
    ) -> Result<Self, CanonicalizationError> {
        Self::between_with_policy(baseline, candidate, FingerprintPolicy::default())
    }

    /// Compares `baseline` with `candidate` using an explicit fingerprint
    /// policy.
    pub fn between_with_policy(
        baseline: &DiagnosticReport,
        candidate: &DiagnosticReport,
        fingerprint_policy: FingerprintPolicy,
    ) -> Result<Self, CanonicalizationError> {
        let baseline_digest = baseline.digest()?;
        let candidate_digest = candidate.digest()?;

        let mut buckets = BTreeMap::<DiagnosticFingerprint, Bucket<'_>>::new();

        for diagnostic in baseline {
            let fingerprint = diagnostic.fingerprint_with_policy(&fingerprint_policy);
            let digest = diagnostic.digest()?;

            buckets
                .entry(fingerprint)
                .or_default()
                .baseline
                .push(Instance { diagnostic, digest });
        }

        for diagnostic in candidate {
            let fingerprint = diagnostic.fingerprint_with_policy(&fingerprint_policy);
            let digest = diagnostic.digest()?;

            buckets
                .entry(fingerprint)
                .or_default()
                .candidate
                .push(Instance { diagnostic, digest });
        }

        let mut entries = Vec::with_capacity(baseline.len().max(candidate.len()));

        for (fingerprint, mut bucket) in buckets {
            bucket.baseline.sort_by_key(|instance| instance.digest);

            bucket.candidate.sort_by_key(|instance| instance.digest);

            classify_bucket(
                fingerprint,
                &bucket.baseline,
                &bucket.candidate,
                &mut entries,
            );
        }

        entries.sort_by(compare_changes);

        let counts = count_entries(&entries);

        Ok(Self {
            baseline_digest,
            candidate_digest,
            fingerprint_policy,
            entries,
            counts,
        })
    }

    /// Canonical digest of the complete baseline report.
    pub const fn baseline_digest(&self) -> ReportDigest {
        self.baseline_digest
    }

    /// Canonical digest of the complete candidate report.
    pub const fn candidate_digest(&self) -> ReportDigest {
        self.candidate_digest
    }

    pub const fn fingerprint_policy(&self) -> &FingerprintPolicy {
        &self.fingerprint_policy
    }

    pub fn entries(&self) -> &[DiagnosticChange] {
        &self.entries
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &DiagnosticChange> {
        self.entries.iter()
    }

    pub fn iter_kind(&self, kind: DeltaKind) -> impl Iterator<Item = &DiagnosticChange> {
        self.entries
            .iter()
            .filter(move |entry| entry.kind() == kind)
    }

    pub const fn counts(&self) -> DeltaCounts {
        self.counts
    }

    pub const fn is_unchanged(&self) -> bool {
        self.counts.is_unchanged()
    }

    pub const fn has_differences(&self) -> bool {
        !self.is_unchanged()
    }

    /// Number of candidate diagnostics at or above `minimum` which cross that
    /// threshold relative to the baseline.
    ///
    /// This counts:
    ///
    /// - new candidate diagnostics at or above `minimum`;
    /// - changed diagnostics whose baseline severity was below `minimum` and
    ///   whose candidate severity is at or above it.
    ///
    /// A diagnostic which was already at or above the threshold in the
    /// baseline is not counted merely because other content changed.
    pub fn introduced_at_or_above(&self, minimum: Severity) -> usize {
        let mut buckets = BTreeMap::<DiagnosticFingerprint, ThresholdCounts>::new();

        for entry in &self.entries {
            let counts = buckets.entry(entry.fingerprint()).or_default();

            if entry
                .baseline()
                .is_some_and(|diagnostic| diagnostic.severity >= minimum)
            {
                counts.baseline += 1;
            }

            if entry
                .candidate()
                .is_some_and(|diagnostic| diagnostic.severity >= minimum)
            {
                counts.candidate += 1;
            }
        }

        buckets
            .values()
            .map(|counts| counts.candidate.saturating_sub(counts.baseline))
            .sum()
    }

    pub fn has_introduced_at_or_above(&self, minimum: Severity) -> bool {
        self.introduced_at_or_above(minimum) != 0
    }

    /// Number of changed diagnostics whose candidate severity is greater than
    /// the baseline severity.
    pub fn severity_increases(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_severity_increase())
            .count()
    }

    /// Number of severity increases whose candidate severity reaches at least
    /// `minimum`.
    pub fn severity_increases_at_or_above(&self, minimum: Severity) -> usize {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .severity_transition()
                    .map(|(baseline, candidate)| candidate > baseline && candidate >= minimum)
                    .unwrap_or(false)
            })
            .count()
    }
}

impl DiagnosticReport {
    /// Compares this report as the candidate against `baseline`.
    pub fn delta_from(&self, baseline: &Self) -> Result<DiagnosticDelta, CanonicalizationError> {
        DiagnosticDelta::between(baseline, self)
    }

    /// Compares this report as the candidate against `baseline` using an
    /// explicit fingerprint policy.
    pub fn delta_from_with_policy(
        &self,
        baseline: &Self,
        fingerprint_policy: FingerprintPolicy,
    ) -> Result<DiagnosticDelta, CanonicalizationError> {
        DiagnosticDelta::between_with_policy(baseline, self, fingerprint_policy)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ThresholdCounts {
    baseline: usize,
    candidate: usize,
}

#[derive(Debug, Clone, Copy)]
struct Instance<'a> {
    diagnostic: &'a Diagnostic,
    digest: DiagnosticDigest,
}

#[derive(Debug, Default)]
struct Bucket<'a> {
    baseline: Vec<Instance<'a>>,
    candidate: Vec<Instance<'a>>,
}

fn classify_bucket(
    fingerprint: DiagnosticFingerprint,
    baseline: &[Instance<'_>],
    candidate: &[Instance<'_>],
    entries: &mut Vec<DiagnosticChange>,
) {
    let mut baseline_index = 0;
    let mut candidate_index = 0;

    let mut unmatched_baseline = Vec::new();
    let mut unmatched_candidate = Vec::new();

    while baseline_index < baseline.len() && candidate_index < candidate.len() {
        let baseline_instance = baseline[baseline_index];
        let candidate_instance = candidate[candidate_index];

        match baseline_instance.digest.cmp(&candidate_instance.digest) {
            Ordering::Equal => {
                entries.push(DiagnosticChange::Persisting {
                    fingerprint,
                    digest: baseline_instance.digest,
                    baseline: baseline_instance.diagnostic.clone(),
                    candidate: candidate_instance.diagnostic.clone(),
                });

                baseline_index += 1;
                candidate_index += 1;
            }

            Ordering::Less => {
                unmatched_baseline.push(baseline_instance);
                baseline_index += 1;
            }

            Ordering::Greater => {
                unmatched_candidate.push(candidate_instance);
                candidate_index += 1;
            }
        }
    }

    unmatched_baseline.extend_from_slice(&baseline[baseline_index..]);
    unmatched_candidate.extend_from_slice(&candidate[candidate_index..]);

    let changed = unmatched_baseline.len().min(unmatched_candidate.len());

    for index in 0..changed {
        let baseline_instance = unmatched_baseline[index];
        let candidate_instance = unmatched_candidate[index];

        entries.push(DiagnosticChange::Changed {
            fingerprint,
            baseline_digest: baseline_instance.digest,
            candidate_digest: candidate_instance.digest,
            baseline: baseline_instance.diagnostic.clone(),
            candidate: candidate_instance.diagnostic.clone(),
        });
    }

    for instance in &unmatched_baseline[changed..] {
        entries.push(DiagnosticChange::Resolved {
            fingerprint,
            baseline_digest: instance.digest,
            baseline: instance.diagnostic.clone(),
        });
    }

    for instance in &unmatched_candidate[changed..] {
        entries.push(DiagnosticChange::New {
            fingerprint,
            candidate_digest: instance.digest,
            candidate: instance.diagnostic.clone(),
        });
    }
}

fn count_entries(entries: &[DiagnosticChange]) -> DeltaCounts {
    let mut counts = DeltaCounts::default();

    for entry in entries {
        match entry.kind() {
            DeltaKind::New => counts.new += 1,
            DeltaKind::Resolved => counts.resolved += 1,
            DeltaKind::Persisting => counts.persisting += 1,
            DeltaKind::Changed => counts.changed += 1,
        }
    }

    counts
}

fn compare_changes(left: &DiagnosticChange, right: &DiagnosticChange) -> Ordering {
    left.fingerprint()
        .cmp(&right.fingerprint())
        .then_with(|| change_rank(left.kind()).cmp(&change_rank(right.kind())))
        .then_with(|| left.baseline_digest().cmp(&right.baseline_digest()))
        .then_with(|| left.candidate_digest().cmp(&right.candidate_digest()))
}

const fn change_rank(kind: DeltaKind) -> u8 {
    match kind {
        DeltaKind::Persisting => 0,
        DeltaKind::Changed => 1,
        DeltaKind::Resolved => 2,
        DeltaKind::New => 3,
    }
}
