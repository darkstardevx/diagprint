use crate::{CanonicalizationError, DiagnosticReport, Severity};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Stable schema for one persisted diagnostic-history run.
pub const DIAGNOSTIC_HISTORY_RUN_V1_SCHEMA: &str = "diagprint.history.run/v1";

/// Stable schema for exported diagnostic-lineage summaries.
pub const DIAGNOSTIC_LINEAGE_V1_SCHEMA: &str = "diagprint.history.lineage/v1";

/// One privacy-light canonical observation of a diagnostic.
///
/// No diagnostic message, source path, source text, attributes, help, notes,
/// causes, or remediation payloads are persisted here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryObservation {
    pub fingerprint: String,
    pub digest: String,
    pub severity: String,
}

/// Severity counts retained for one historical run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistorySeverityCounts {
    pub trace: usize,
    pub debug: usize,
    pub info: usize,
    pub warning: usize,
    pub error: usize,
    pub fatal: usize,
}

impl HistorySeverityCounts {
    pub const fn total(self) -> usize {
        self.trace + self.debug + self.info + self.warning + self.error + self.fatal
    }

    pub const fn failures(self) -> usize {
        self.error + self.fatal
    }
}

/// One immutable persisted diagnostic state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticHistoryRun {
    pub schema: String,

    pub index: usize,
    pub label: String,

    pub report_digest: String,

    pub diagnostics: usize,

    pub severity: HistorySeverityCounts,

    pub observations: Vec<HistoryObservation>,
}

impl DiagnosticHistoryRun {
    pub fn from_report(
        index: usize,
        label: impl Into<String>,
        report: &DiagnosticReport,
    ) -> Result<Self, DiagnosticHistoryError> {
        let report_digest = report
            .digest()
            .map_err(DiagnosticHistoryError::Canonicalization)?
            .qualified();

        let counts = report.counts();

        let mut observations = Vec::with_capacity(report.len());

        for diagnostic in report.iter() {
            observations.push(HistoryObservation {
                fingerprint: diagnostic.fingerprint().qualified(),

                digest: diagnostic
                    .digest()
                    .map_err(DiagnosticHistoryError::Canonicalization)?
                    .qualified(),

                severity: severity_name(diagnostic.severity).to_owned(),
            });
        }

        observations.sort_by(|left, right| {
            left.fingerprint
                .cmp(&right.fingerprint)
                .then_with(|| left.digest.cmp(&right.digest))
                .then_with(|| left.severity.cmp(&right.severity))
        });

        Ok(Self {
            schema: DIAGNOSTIC_HISTORY_RUN_V1_SCHEMA.to_owned(),

            index,

            label: label.into(),

            report_digest,

            diagnostics: report.len(),

            severity: HistorySeverityCounts {
                trace: counts.trace,
                debug: counts.debug,
                info: counts.info,
                warning: counts.warning,
                error: counts.error,
                fatal: counts.fatal,
            },

            observations,
        })
    }
}

/// Counts semantic diagnostic transitions between two history runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryDeltaCounts {
    pub new: usize,
    pub resolved: usize,
    pub persisting: usize,
    pub changed: usize,
}

impl HistoryDeltaCounts {
    pub const fn differences(self) -> usize {
        self.new + self.resolved + self.changed
    }

    pub const fn is_unchanged(self) -> bool {
        self.differences() == 0
    }

    pub const fn baseline_total(self) -> usize {
        self.resolved + self.persisting + self.changed
    }

    pub const fn candidate_total(self) -> usize {
        self.new + self.persisting + self.changed
    }
}

/// Semantic transition between two persisted history runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticHistoryTransition {
    pub from_run: usize,
    pub to_run: usize,

    pub from_report: String,
    pub to_report: String,

    pub counts: HistoryDeltaCounts,

    pub introduced_errors: usize,
    pub severity_increases: usize,
}

impl DiagnosticHistoryTransition {
    pub const fn has_differences(&self) -> bool {
        !self.counts.is_unchanged()
    }
}

/// One transition in the history of a single logical diagnostic fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLineageStep {
    pub from_run: Option<usize>,
    pub to_run: usize,

    pub counts: HistoryDeltaCounts,

    pub severity_increases: usize,

    pub active_instances: usize,
}

/// Cross-run history for one logical diagnostic fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLineage {
    pub schema: String,

    pub fingerprint: String,

    pub steps: Vec<DiagnosticLineageStep>,
}

impl DiagnosticLineage {
    pub fn first_seen_run(&self) -> Option<usize> {
        self.steps
            .iter()
            .find(|step| step.active_instances != 0)
            .map(|step| step.to_run)
    }

    pub fn last_seen_run(&self) -> Option<usize> {
        self.steps
            .iter()
            .rfind(|step| step.active_instances != 0)
            .map(|step| step.to_run)
    }

    pub fn active_in_latest(&self) -> bool {
        self.steps
            .last()
            .is_some_and(|step| step.active_instances != 0)
    }

    pub fn reappeared_after_absence(&self) -> bool {
        let mut seen = false;
        let mut absent_after_seen = false;

        for step in &self.steps {
            if step.active_instances != 0 {
                if absent_after_seen {
                    return true;
                }

                seen = true;
            } else if seen {
                absent_after_seen = true;
            }
        }

        false
    }

    pub fn changes(&self) -> usize {
        self.steps.iter().map(|step| step.counts.changed).sum()
    }

    pub fn resolutions(&self) -> usize {
        self.steps.iter().map(|step| step.counts.resolved).sum()
    }

    pub fn appearances(&self) -> usize {
        self.steps.iter().map(|step| step.counts.new).sum()
    }
}

/// Append-only persistent diagnostic history.
///
/// Every run is stored as an immutable `run-XXXXXX.json` file. Existing run
/// files are never rewritten.
#[derive(Debug, Clone)]
pub struct DiagnosticHistory {
    directory: PathBuf,
    runs: Vec<DiagnosticHistoryRun>,
}

impl DiagnosticHistory {
    /// Opens or creates an append-only diagnostic-history directory.
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, DiagnosticHistoryError> {
        let directory = directory.as_ref().to_path_buf();

        fs::create_dir_all(&directory).map_err(|source| {
            io_error("create diagnostic history directory", &directory, source)
        })?;

        let mut discovered = Vec::new();

        for entry in fs::read_dir(&directory)
            .map_err(|source| io_error("read diagnostic history directory", &directory, source))?
        {
            let entry = entry
                .map_err(|source| io_error("read diagnostic history entry", &directory, source))?;

            let path = entry.path();

            if !entry
                .file_type()
                .map_err(|source| io_error("read diagnostic history entry type", &path, source))?
                .is_file()
            {
                continue;
            }

            let Some(index) = parse_run_filename(
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default(),
            ) else {
                continue;
            };

            discovered.push((index, path));
        }

        discovered.sort_by_key(|(index, _)| *index);

        let mut runs = Vec::with_capacity(discovered.len());

        let mut seen = BTreeSet::new();

        for (expected_index, (file_index, path)) in discovered.into_iter().enumerate() {
            if !seen.insert(file_index) {
                return Err(DiagnosticHistoryError::DuplicateRunIndex { index: file_index });
            }

            if file_index != expected_index {
                return Err(DiagnosticHistoryError::InvalidRunSequence {
                    expected: expected_index,

                    actual: file_index,
                });
            }

            let bytes = fs::read(&path)
                .map_err(|source| io_error("read diagnostic history run", &path, source))?;

            let run: DiagnosticHistoryRun =
                serde_json::from_slice(&bytes).map_err(DiagnosticHistoryError::Json)?;

            if run.schema != DIAGNOSTIC_HISTORY_RUN_V1_SCHEMA {
                return Err(DiagnosticHistoryError::UnsupportedSchema {
                    path,

                    schema: run.schema,
                });
            }

            if run.index != file_index {
                return Err(DiagnosticHistoryError::RunIndexMismatch {
                    file_index,

                    record_index: run.index,
                });
            }

            runs.push(run);
        }

        Ok(Self { directory, runs })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn len(&self) -> usize {
        self.runs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    pub fn runs(&self) -> &[DiagnosticHistoryRun] {
        &self.runs
    }

    pub fn latest(&self) -> Option<&DiagnosticHistoryRun> {
        self.runs.last()
    }

    /// Appends one immutable report state to this history.
    pub fn append_report(
        &mut self,
        label: impl Into<String>,
        report: &DiagnosticReport,
    ) -> Result<&DiagnosticHistoryRun, DiagnosticHistoryError> {
        let index = self.runs.len();

        let run = DiagnosticHistoryRun::from_report(index, label, report)?;

        let path = self.directory.join(run_filename(index));

        let bytes = serde_json::to_vec_pretty(&run).map_err(DiagnosticHistoryError::Json)?;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| io_error("create diagnostic history run", &path, source))?;

        file.write_all(&bytes)
            .map_err(|source| io_error("write diagnostic history run", &path, source))?;

        file.write_all(b"\n")
            .map_err(|source| io_error("finish diagnostic history run", &path, source))?;

        file.flush()
            .map_err(|source| io_error("flush diagnostic history run", &path, source))?;

        file.sync_all()
            .map_err(|source| io_error("synchronize diagnostic history run", &path, source))?;

        self.runs.push(run);

        Ok(&self.runs[index])
    }

    pub fn transition(
        &self,
        from: usize,
        to: usize,
    ) -> Result<DiagnosticHistoryTransition, DiagnosticHistoryError> {
        let baseline = self
            .runs
            .get(from)
            .ok_or(DiagnosticHistoryError::RunOutOfRange {
                index: from,
                len: self.runs.len(),
            })?;

        let candidate = self
            .runs
            .get(to)
            .ok_or(DiagnosticHistoryError::RunOutOfRange {
                index: to,
                len: self.runs.len(),
            })?;

        Ok(compare_runs(baseline, candidate))
    }

    pub fn latest_transition(
        &self,
    ) -> Result<Option<DiagnosticHistoryTransition>, DiagnosticHistoryError> {
        if self.runs.len() < 2 {
            return Ok(None);
        }

        let to = self.runs.len() - 1;

        let from = to - 1;

        self.transition(from, to).map(Some)
    }

    /// Returns all distinct logical diagnostic fingerprints ever recorded.
    pub fn fingerprints(&self) -> Vec<String> {
        let mut values = BTreeSet::new();

        for run in &self.runs {
            for observation in &run.observations {
                values.insert(observation.fingerprint.clone());
            }
        }

        values.into_iter().collect()
    }

    /// Builds the cross-run lineage for one canonical diagnostic fingerprint.
    pub fn lineage(&self, fingerprint: &str) -> DiagnosticLineage {
        let mut steps = Vec::with_capacity(self.runs.len());

        if let Some(first) = self.runs.first() {
            let active = observations_for(first, fingerprint);

            steps.push(DiagnosticLineageStep {
                from_run: None,
                to_run: 0,

                counts: HistoryDeltaCounts {
                    new: active.len(),

                    ..HistoryDeltaCounts::default()
                },

                severity_increases: 0,

                active_instances: active.len(),
            });
        }

        for to in 1..self.runs.len() {
            let from = to - 1;

            let baseline = observations_for(&self.runs[from], fingerprint);

            let candidate = observations_for(&self.runs[to], fingerprint);

            let comparison = compare_observations(&baseline, &candidate);

            steps.push(DiagnosticLineageStep {
                from_run: Some(from),

                to_run: to,

                counts: comparison.counts,

                severity_increases: comparison.severity_increases,

                active_instances: candidate.len(),
            });
        }

        DiagnosticLineage {
            schema: DIAGNOSTIC_LINEAGE_V1_SCHEMA.to_owned(),

            fingerprint: fingerprint.to_owned(),

            steps,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ObservationComparison {
    counts: HistoryDeltaCounts,
    severity_increases: usize,
    introduced_errors: usize,
}

fn compare_runs(
    baseline: &DiagnosticHistoryRun,
    candidate: &DiagnosticHistoryRun,
) -> DiagnosticHistoryTransition {
    let mut fingerprints = BTreeSet::new();

    for observation in &baseline.observations {
        fingerprints.insert(observation.fingerprint.as_str());
    }

    for observation in &candidate.observations {
        fingerprints.insert(observation.fingerprint.as_str());
    }

    let mut counts = HistoryDeltaCounts::default();

    let mut severity_increases = 0usize;

    let mut introduced_errors = 0usize;

    for fingerprint in fingerprints {
        let baseline_entries = observations_for(baseline, fingerprint);

        let candidate_entries = observations_for(candidate, fingerprint);

        let comparison = compare_observations(&baseline_entries, &candidate_entries);

        counts.new += comparison.counts.new;

        counts.resolved += comparison.counts.resolved;

        counts.persisting += comparison.counts.persisting;

        counts.changed += comparison.counts.changed;

        severity_increases += comparison.severity_increases;

        introduced_errors += comparison.introduced_errors;
    }

    DiagnosticHistoryTransition {
        from_run: baseline.index,

        to_run: candidate.index,

        from_report: baseline.report_digest.clone(),

        to_report: candidate.report_digest.clone(),

        counts,

        introduced_errors,

        severity_increases,
    }
}

fn compare_observations(
    baseline: &[&HistoryObservation],
    candidate: &[&HistoryObservation],
) -> ObservationComparison {
    let mut baseline_used = vec![false; baseline.len()];

    let mut candidate_used = vec![false; candidate.len()];

    let mut counts = HistoryDeltaCounts::default();

    // Pair exact semantic digests first.
    for (candidate_index, candidate_entry) in candidate.iter().enumerate() {
        let Some(baseline_index) =
            baseline
                .iter()
                .enumerate()
                .find_map(|(baseline_index, baseline_entry)| {
                    (!baseline_used[baseline_index]
                        && baseline_entry.digest == candidate_entry.digest)
                        .then_some(baseline_index)
                })
        else {
            continue;
        };

        baseline_used[baseline_index] = true;

        candidate_used[candidate_index] = true;

        counts.persisting += 1;
    }

    let mut remaining_baseline = baseline
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| (!baseline_used[index]).then_some(*observation))
        .collect::<Vec<_>>();

    let mut remaining_candidate = candidate
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| (!candidate_used[index]).then_some(*observation))
        .collect::<Vec<_>>();

    remaining_baseline.sort_by(observation_order);

    remaining_candidate.sort_by(observation_order);

    let changed = remaining_baseline.len().min(remaining_candidate.len());

    counts.changed += changed;

    let mut severity_increases = 0usize;

    for index in 0..changed {
        if severity_rank(&remaining_candidate[index].severity)
            > severity_rank(&remaining_baseline[index].severity)
        {
            severity_increases += 1;
        }
    }

    counts.resolved += remaining_baseline.len().saturating_sub(changed);

    counts.new += remaining_candidate.len().saturating_sub(changed);

    let baseline_errors = baseline
        .iter()
        .filter(|observation| severity_rank(&observation.severity) >= severity_rank("error"))
        .count();

    let candidate_errors = candidate
        .iter()
        .filter(|observation| severity_rank(&observation.severity) >= severity_rank("error"))
        .count();

    ObservationComparison {
        counts,

        severity_increases,

        introduced_errors: candidate_errors.saturating_sub(baseline_errors),
    }
}

fn observations_for<'a>(
    run: &'a DiagnosticHistoryRun,
    fingerprint: &str,
) -> Vec<&'a HistoryObservation> {
    run.observations
        .iter()
        .filter(|observation| observation.fingerprint == fingerprint)
        .collect()
}

fn observation_order(
    left: &&HistoryObservation,
    right: &&HistoryObservation,
) -> std::cmp::Ordering {
    left.digest
        .cmp(&right.digest)
        .then_with(|| left.severity.cmp(&right.severity))
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Trace => "trace",
        Severity::Debug => "debug",
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "fatal",
    }
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "trace" => 0,
        "debug" => 1,
        "info" => 2,
        "warning" => 3,
        "error" => 4,
        "fatal" => 5,
        _ => 0,
    }
}

fn run_filename(index: usize) -> String {
    format!("run-{index:06}.json",)
}

fn parse_run_filename(value: &str) -> Option<usize> {
    value
        .strip_prefix("run-")?
        .strip_suffix(".json")?
        .parse()
        .ok()
}

/// Diagnostic-history persistence or integrity failure.
#[derive(Debug)]
pub enum DiagnosticHistoryError {
    Canonicalization(CanonicalizationError),

    Json(serde_json::Error),

    RunOutOfRange {
        index: usize,
        len: usize,
    },

    InvalidRunSequence {
        expected: usize,
        actual: usize,
    },

    DuplicateRunIndex {
        index: usize,
    },

    RunIndexMismatch {
        file_index: usize,
        record_index: usize,
    },

    UnsupportedSchema {
        path: PathBuf,
        schema: String,
    },

    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for DiagnosticHistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonicalization(error) => write!(
                formatter,
                "diagnostic history canonicalization failed: {error}",
            ),

            Self::Json(error) => write!(
                formatter,
                "diagnostic history JSON operation failed: {error}",
            ),

            Self::RunOutOfRange { index, len } => write!(
                formatter,
                "diagnostic history run index {index} is out of range for {len} recorded run(s)",
            ),

            Self::InvalidRunSequence { expected, actual } => write!(
                formatter,
                "diagnostic history run sequence is incomplete: expected run {expected}, found run {actual}",
            ),

            Self::DuplicateRunIndex { index } => write!(
                formatter,
                "diagnostic history contains duplicate run index {index}",
            ),

            Self::RunIndexMismatch {
                file_index,
                record_index,
            } => write!(
                formatter,
                "diagnostic history filename identifies run {file_index}, but record identifies run {record_index}",
            ),

            Self::UnsupportedSchema { path, schema } => write!(
                formatter,
                "unsupported diagnostic history schema {schema:?} in {}",
                path.display(),
            ),

            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display(),
            ),
        }
    }
}

impl Error for DiagnosticHistoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonicalization(error) => Some(error),

            Self::Json(error) => Some(error),

            Self::Io { source, .. } => Some(source),

            Self::RunOutOfRange { .. }
            | Self::InvalidRunSequence { .. }
            | Self::DuplicateRunIndex { .. }
            | Self::RunIndexMismatch { .. }
            | Self::UnsupportedSchema { .. } => None,
        }
    }
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> DiagnosticHistoryError {
    DiagnosticHistoryError::Io {
        operation,

        path: path.to_path_buf(),

        source,
    }
}
