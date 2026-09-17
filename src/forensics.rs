use crate::history::{DiagnosticHistory, DiagnosticLineage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stable schema for privacy-light diagnostic forensic case files.
///
/// V1 is intentionally evidence-based. It records only information derivable
/// from the verified diagnostic-history chain and does not infer source-control
/// blame, root cause, authorship, or remediation success.
pub const DIAGNOSTIC_CASE_FILE_V1_SCHEMA: &str = "diagprint.forensics.case-file/v1";

/// Current lifecycle state of one logical diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCaseStatus {
    /// The fingerprint is present in the latest history run.
    Active,

    /// The fingerprint was observed historically but is absent from the latest
    /// history run.
    Resolved,
}

impl DiagnosticCaseStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Resolved => "resolved",
        }
    }
}

/// Evidence for one history run in which the diagnostic was actually present.
///
/// This representation deliberately retains no diagnostic message, source
/// path, source text, arbitrary attribute, help, note, cause, or remediation
/// payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCaseRun {
    pub run_index: usize,

    pub label: String,

    pub report_digest: String,

    pub run_digest: String,

    pub instances: usize,

    /// Per-severity instance counts for this fingerprint in this run.
    ///
    /// A string-keyed map deliberately remains forward-compatible with future
    /// severity names stored by a later history schema.
    pub severities: BTreeMap<String, usize>,

    /// Unique canonical diagnostic content digests observed for this
    /// fingerprint in this run.
    pub diagnostic_digests: Vec<String>,
}

/// One contiguous period during which a logical diagnostic was present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEpisode {
    /// One-based episode number within this case file.
    pub episode: usize,

    pub started_run: usize,

    pub last_active_run: usize,

    /// First run after this episode in which the fingerprint was absent.
    ///
    /// `None` means the episode remains active in the latest history run.
    pub resolved_run: Option<usize>,

    pub observed_runs: usize,

    pub instances: usize,

    pub peak_instances: usize,
}

/// Privacy-light forensic summary for one logical diagnostic fingerprint.
///
/// A case file is a deterministic interpretation of an already verified
/// [`DiagnosticHistory`]. It does not introduce a second persistence format or
/// database.
///
/// Case-file v1 intentionally answers only questions supported by retained
/// history evidence:
///
/// - when the fingerprint first and last appeared in run order;
/// - whether it is active now;
/// - how many contiguous episodes it has had;
/// - whether it disappeared and later reappeared;
/// - how many instances were introduced, resolved, or changed;
/// - whether severity increased;
/// - which history runs and canonical diagnostic digests support the result;
/// - which cryptographic history-chain head anchors the evidence.
///
/// Source-control blame and causal inference are deliberately outside v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCaseFile {
    pub schema: String,

    pub fingerprint: String,

    pub status: DiagnosticCaseStatus,

    /// Total number of runs in the verified history, including runs where this
    /// fingerprint was absent.
    pub history_runs: usize,

    /// Number of runs containing at least one instance of this fingerprint.
    pub observed_runs: usize,

    /// Total number of matching diagnostic instances across all retained runs.
    pub observed_instances: usize,

    /// Number of distinct canonical diagnostic content digests seen for this
    /// logical fingerprint.
    pub unique_digests: usize,

    pub first_seen_run: usize,

    pub last_seen_run: usize,

    /// Number of matching instances in the latest history run.
    pub active_instances: usize,

    /// Number of instances classified as newly introduced across lineage
    /// transitions.
    pub introduced_instances: usize,

    /// Number of instances classified as resolved across lineage transitions.
    pub resolved_instances: usize,

    /// Number of same-fingerprint instances whose canonical content changed.
    pub changed_instances: usize,

    pub severity_increases: usize,

    /// Number of times a diagnostic began a new episode after at least one
    /// previous episode had ended.
    pub reappearances: usize,

    /// Current cryptographic history-chain head.
    pub chain_head: Option<String>,

    pub episodes: Vec<DiagnosticEpisode>,

    /// Evidence-bearing runs in chronological run order.
    pub evidence: Vec<DiagnosticCaseRun>,
}

impl DiagnosticCaseFile {
    /// Builds a forensic case file from one verified in-memory history.
    ///
    /// Returns `None` when the fingerprint has never been observed.
    pub fn from_history(history: &DiagnosticHistory, fingerprint: &str) -> Option<Self> {
        let lineage = history.lineage(fingerprint);

        let first_seen_run = lineage.first_seen_run()?;
        let last_seen_run = lineage.last_seen_run()?;

        let status = if lineage.active_in_latest() {
            DiagnosticCaseStatus::Active
        } else {
            DiagnosticCaseStatus::Resolved
        };

        let active_instances = lineage.steps.last().map_or(0, |step| step.active_instances);

        let severity_increases = lineage
            .steps
            .iter()
            .map(|step| step.severity_increases)
            .sum();

        let mut unique_digests = BTreeSet::new();

        let mut evidence = Vec::new();

        for run in history.runs() {
            let mut instances = 0usize;

            let mut severities = BTreeMap::<String, usize>::new();

            let mut run_digests = BTreeSet::<String>::new();

            for observation in run
                .observations
                .iter()
                .filter(|observation| observation.fingerprint == fingerprint)
            {
                instances = instances.saturating_add(1);

                *severities.entry(observation.severity.clone()).or_default() += 1;

                run_digests.insert(observation.digest.clone());

                unique_digests.insert(observation.digest.clone());
            }

            if instances == 0 {
                continue;
            }

            evidence.push(DiagnosticCaseRun {
                run_index: run.index,

                label: run.label.clone(),

                report_digest: run.report_digest.clone(),

                run_digest: run.run_digest.to_string(),

                instances,

                severities,

                diagnostic_digests: run_digests.into_iter().collect(),
            });
        }

        let episodes = build_episodes(&lineage);

        let observed_instances = evidence.iter().map(|run| run.instances).sum();

        let reappearances = episodes.len().saturating_sub(1);

        Some(Self {
            schema: DIAGNOSTIC_CASE_FILE_V1_SCHEMA.to_owned(),

            fingerprint: fingerprint.to_owned(),

            status,

            history_runs: history.len(),

            observed_runs: evidence.len(),

            observed_instances,

            unique_digests: unique_digests.len(),

            first_seen_run,

            last_seen_run,

            active_instances,

            introduced_instances: lineage.appearances(),

            resolved_instances: lineage.resolutions(),

            changed_instances: lineage.changes(),

            severity_increases,

            reappearances,

            chain_head: history.head_digest().map(|digest| digest.to_string()),

            episodes,

            evidence,
        })
    }

    pub const fn is_active(&self) -> bool {
        matches!(self.status, DiagnosticCaseStatus::Active)
    }

    pub const fn is_resolved(&self) -> bool {
        matches!(self.status, DiagnosticCaseStatus::Resolved)
    }
}

impl DiagnosticHistory {
    /// Builds a privacy-light forensic case file for one exact canonical
    /// diagnostic fingerprint.
    ///
    /// Prefix resolution belongs to presentation layers such as the diagprint
    /// CLI; this API deliberately requires the complete fingerprint.
    pub fn case_file(&self, fingerprint: &str) -> Option<DiagnosticCaseFile> {
        DiagnosticCaseFile::from_history(self, fingerprint)
    }
}

fn build_episodes(lineage: &DiagnosticLineage) -> Vec<DiagnosticEpisode> {
    let mut episodes = Vec::new();

    let mut current: Option<DiagnosticEpisode> = None;

    for step in &lineage.steps {
        if step.active_instances != 0 {
            match current.as_mut() {
                Some(episode) => {
                    episode.last_active_run = step.to_run;

                    episode.observed_runs = episode.observed_runs.saturating_add(1);

                    episode.instances = episode.instances.saturating_add(step.active_instances);

                    episode.peak_instances = episode.peak_instances.max(step.active_instances);
                }

                None => {
                    current = Some(DiagnosticEpisode {
                        episode: episodes.len() + 1,

                        started_run: step.to_run,

                        last_active_run: step.to_run,

                        resolved_run: None,

                        observed_runs: 1,

                        instances: step.active_instances,

                        peak_instances: step.active_instances,
                    });
                }
            }

            continue;
        }

        if let Some(mut episode) = current.take() {
            episode.resolved_run = Some(step.to_run);

            episodes.push(episode);
        }
    }

    if let Some(episode) = current {
        episodes.push(episode);
    }

    episodes
}
