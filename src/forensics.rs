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

/// Stable schema for run-by-run diagnostic forensic timelines.
///
/// Timeline v1 is derived entirely from verified diagnostic history. It
/// preserves the evidence-only boundary established by case-file v1.
pub const DIAGNOSTIC_TIMELINE_V1_SCHEMA: &str = "diagprint.forensics.timeline/v1";

/// Presence state of one logical diagnostic in one retained history run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticTimelinePhase {
    /// The run occurred before this fingerprint had ever been observed.
    Unseen,

    /// One or more matching instances are present in this run.
    Active,

    /// The fingerprint had previously been observed but is absent in this run.
    Absent,
}

impl DiagnosticTimelinePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unseen => "unseen",
            Self::Active => "active",
            Self::Absent => "absent",
        }
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Notable lifecycle event occurring at one timeline run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticTimelineEvent {
    /// First retained observation of this fingerprint.
    FirstSeen,

    /// The diagnostic remained active without a more significant transition.
    Persisting,

    /// Canonical content changed while logical fingerprint identity remained.
    Changed,

    /// At least one same-fingerprint instance increased in severity.
    SeverityIncreased,

    /// The previous run contained the fingerprint and this run does not.
    Resolved,

    /// The fingerprint became active after at least one absent run.
    Reappeared,
}

impl DiagnosticTimelineEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstSeen => "first_seen",
            Self::Persisting => "persisting",
            Self::Changed => "changed",
            Self::SeverityIncreased => "severity_increased",
            Self::Resolved => "resolved",
            Self::Reappeared => "reappeared",
        }
    }
}

/// One complete run in a diagnostic forensic timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticTimelineRun {
    pub run_index: usize,

    pub label: String,

    pub phase: DiagnosticTimelinePhase,

    /// One-based active episode number.
    ///
    /// Absent and unseen runs carry `None`.
    pub episode: Option<usize>,

    pub instances: usize,

    pub severities: BTreeMap<String, usize>,

    pub diagnostic_digests: Vec<String>,

    pub introduced_instances: usize,

    pub resolved_instances: usize,

    pub persisting_instances: usize,

    pub changed_instances: usize,

    pub severity_increases: usize,

    pub events: Vec<DiagnosticTimelineEvent>,
}

/// Classification for one contiguous run window where the diagnostic is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCleanWindowKind {
    /// Runs retained before the diagnostic was ever observed.
    BeforeFirstSeen,

    /// Absence separating two active episodes.
    BetweenEpisodes,

    /// Absence after the final retained active episode.
    AfterResolution,
}

impl DiagnosticCleanWindowKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BeforeFirstSeen => "before_first_seen",
            Self::BetweenEpisodes => "between_episodes",
            Self::AfterResolution => "after_resolution",
        }
    }
}

/// One contiguous history interval where the fingerprint is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCleanWindow {
    pub kind: DiagnosticCleanWindowKind,

    pub start_run: usize,

    pub end_run: usize,

    pub runs: usize,
}

/// Complete run-by-run forensic lifecycle for one logical fingerprint.
///
/// Unlike [`DiagnosticCaseFile`], which summarizes the lifecycle, a timeline
/// retains one row for every history run, including runs where the fingerprint
/// was not present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticTimeline {
    pub schema: String,

    pub fingerprint: String,

    pub status: DiagnosticCaseStatus,

    pub history_runs: usize,

    pub first_seen_run: usize,

    pub last_seen_run: usize,

    pub active_instances: usize,

    pub reappearances: usize,

    pub chain_head: Option<String>,

    pub runs: Vec<DiagnosticTimelineRun>,

    pub clean_windows: Vec<DiagnosticCleanWindow>,
}

impl DiagnosticTimeline {
    /// Builds a deterministic timeline from one verified in-memory history.
    ///
    /// Returns `None` when the fingerprint has never been observed.
    pub fn from_history(history: &DiagnosticHistory, fingerprint: &str) -> Option<Self> {
        let case = history.case_file(fingerprint)?;
        let lineage = history.lineage(fingerprint);

        let mut runs = Vec::with_capacity(history.len());

        let mut seen_before = false;
        let mut previous_active = false;
        let mut episode = 0usize;

        for run in history.runs() {
            let step = lineage.steps.iter().find(|step| step.to_run == run.index)?;

            let mut instances = 0usize;

            let mut severities = BTreeMap::<String, usize>::new();

            let mut diagnostic_digests = BTreeSet::<String>::new();

            for observation in run
                .observations
                .iter()
                .filter(|observation| observation.fingerprint == fingerprint)
            {
                instances = instances.saturating_add(1);

                *severities.entry(observation.severity.clone()).or_default() += 1;

                diagnostic_digests.insert(observation.digest.clone());
            }

            let active = instances != 0;

            let phase = if active {
                DiagnosticTimelinePhase::Active
            } else if seen_before {
                DiagnosticTimelinePhase::Absent
            } else {
                DiagnosticTimelinePhase::Unseen
            };

            if active && !previous_active {
                episode = episode.saturating_add(1);
            }

            let current_episode = active.then_some(episode);

            let mut events = Vec::new();

            if active {
                if !seen_before {
                    events.push(DiagnosticTimelineEvent::FirstSeen);
                } else if !previous_active {
                    events.push(DiagnosticTimelineEvent::Reappeared);
                }

                if step.counts.changed != 0 {
                    events.push(DiagnosticTimelineEvent::Changed);
                }

                if step.severity_increases != 0 {
                    events.push(DiagnosticTimelineEvent::SeverityIncreased);
                }

                if previous_active && events.is_empty() {
                    events.push(DiagnosticTimelineEvent::Persisting);
                }
            } else if previous_active {
                events.push(DiagnosticTimelineEvent::Resolved);
            }

            runs.push(DiagnosticTimelineRun {
                run_index: run.index,

                label: run.label.clone(),

                phase,

                episode: current_episode,

                instances,

                severities,

                diagnostic_digests: diagnostic_digests.into_iter().collect(),

                introduced_instances: step.counts.new,

                resolved_instances: step.counts.resolved,

                persisting_instances: step.counts.persisting,

                changed_instances: step.counts.changed,

                severity_increases: step.severity_increases,

                events,
            });

            if active {
                seen_before = true;
            }

            previous_active = active;
        }

        let clean_windows = build_clean_windows(&runs, case.first_seen_run);

        Some(Self {
            schema: DIAGNOSTIC_TIMELINE_V1_SCHEMA.to_owned(),

            fingerprint: fingerprint.to_owned(),

            status: case.status,

            history_runs: history.len(),

            first_seen_run: case.first_seen_run,

            last_seen_run: case.last_seen_run,

            active_instances: case.active_instances,

            reappearances: case.reappearances,

            chain_head: case.chain_head,

            runs,

            clean_windows,
        })
    }

    pub fn active_runs(&self) -> usize {
        self.runs.iter().filter(|run| run.phase.is_active()).count()
    }

    pub fn absent_runs(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| matches!(run.phase, DiagnosticTimelinePhase::Absent))
            .count()
    }

    pub fn unseen_runs(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| matches!(run.phase, DiagnosticTimelinePhase::Unseen))
            .count()
    }
}

impl DiagnosticHistory {
    /// Builds a complete privacy-light timeline for one exact canonical
    /// diagnostic fingerprint.
    pub fn timeline(&self, fingerprint: &str) -> Option<DiagnosticTimeline> {
        DiagnosticTimeline::from_history(self, fingerprint)
    }
}

fn build_clean_windows(
    runs: &[DiagnosticTimelineRun],
    first_seen_run: usize,
) -> Vec<DiagnosticCleanWindow> {
    let mut windows = Vec::new();

    let mut index = 0usize;

    while index < runs.len() {
        if runs[index].phase.is_active() {
            index += 1;
            continue;
        }

        let start = index;

        while index < runs.len() && !runs[index].phase.is_active() {
            index += 1;
        }

        let end = index - 1;

        let later_active = runs.iter().skip(index).any(|run| run.phase.is_active());

        let kind = if runs[end].run_index < first_seen_run {
            DiagnosticCleanWindowKind::BeforeFirstSeen
        } else if later_active {
            DiagnosticCleanWindowKind::BetweenEpisodes
        } else {
            DiagnosticCleanWindowKind::AfterResolution
        };

        windows.push(DiagnosticCleanWindow {
            kind,

            start_run: runs[start].run_index,

            end_run: runs[end].run_index,

            runs: end - start + 1,
        });
    }

    windows
}
