use crate::{ArtifactDigest, DiagnosticHistory, DiagnosticHistoryRun};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Stable schema for Git provenance bound to one immutable diagnostic-history
/// run.
pub const GIT_PROVENANCE_V1_SCHEMA: &str = "diagprint.forensics.git-provenance/v1";

const GIT_PROVENANCE_DIRECTORY: &str = "git-provenance";

/// Evidence strength for a Git/history binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitProvenanceBinding {
    /// diagprint captured this Git revision while the worktree was clean before
    /// the scan and verified that it remained clean and unchanged after the
    /// scan completed.
    CapturedClean,

    /// A user explicitly associated an existing history run with a Git commit
    /// after the fact.
    UserAsserted,
}

impl GitProvenanceBinding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapturedClean => "captured_clean",
            Self::UserAsserted => "user_asserted",
        }
    }
}

/// Immutable Git provenance for exactly one diagnostic-history run.
///
/// The record deliberately contains repository object identities rather than
/// mutable branch names, remote URLs, author identity, or absolute paths.
///
/// Author, date, subject, and diff context are queried from the local Git object
/// database by presentation tools such as `diagprint blame`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitProvenanceRecord {
    pub schema: String,

    pub run_index: usize,

    pub history_run_digest: String,

    pub report_digest: String,

    pub binding: GitProvenanceBinding,

    pub commit: String,

    pub tree: String,

    pub parents: Vec<String>,

    /// SHA-256 identity of the deterministic compact JSON representation of
    /// every preceding field.
    pub record_digest: ArtifactDigest,
}

#[derive(Serialize)]
struct GitProvenanceDigestPayload<'a> {
    schema: &'a str,

    run_index: usize,

    history_run_digest: &'a str,

    report_digest: &'a str,

    binding: GitProvenanceBinding,

    commit: &'a str,

    tree: &'a str,

    parents: &'a [String],
}

impl GitProvenanceRecord {
    pub fn captured_clean(
        run: &DiagnosticHistoryRun,
        commit: impl Into<String>,
        tree: impl Into<String>,
        parents: Vec<String>,
    ) -> Result<Self, GitProvenanceError> {
        Self::new(
            run,
            GitProvenanceBinding::CapturedClean,
            commit.into(),
            tree.into(),
            parents,
        )
    }

    pub fn user_asserted(
        run: &DiagnosticHistoryRun,
        commit: impl Into<String>,
        tree: impl Into<String>,
        parents: Vec<String>,
    ) -> Result<Self, GitProvenanceError> {
        Self::new(
            run,
            GitProvenanceBinding::UserAsserted,
            commit.into(),
            tree.into(),
            parents,
        )
    }

    fn new(
        run: &DiagnosticHistoryRun,
        binding: GitProvenanceBinding,
        commit: String,
        tree: String,
        parents: Vec<String>,
    ) -> Result<Self, GitProvenanceError> {
        validate_object_id("commit", &commit)?;
        validate_object_id("tree", &tree)?;

        for parent in &parents {
            validate_object_id("parent", parent)?;
        }

        let schema = GIT_PROVENANCE_V1_SCHEMA.to_owned();

        let history_run_digest = run.run_digest.to_string();

        let report_digest = run.report_digest.clone();

        let payload = GitProvenanceDigestPayload {
            schema: &schema,
            run_index: run.index,
            history_run_digest: &history_run_digest,
            report_digest: &report_digest,
            binding,
            commit: &commit,
            tree: &tree,
            parents: &parents,
        };

        let record_digest = compute_record_digest(&payload)?;

        Ok(Self {
            schema,

            run_index: run.index,

            history_run_digest,

            report_digest,

            binding,

            commit,

            tree,

            parents,

            record_digest,
        })
    }

    /// Verifies this record's own schema, object identifiers, and digest.
    pub fn verify_record(&self) -> Result<(), GitProvenanceError> {
        if self.schema != GIT_PROVENANCE_V1_SCHEMA {
            return Err(GitProvenanceError::UnsupportedSchema {
                schema: self.schema.clone(),
            });
        }

        validate_object_id("commit", &self.commit)?;

        validate_object_id("tree", &self.tree)?;

        for parent in &self.parents {
            validate_object_id("parent", parent)?;
        }

        let payload = GitProvenanceDigestPayload {
            schema: &self.schema,
            run_index: self.run_index,
            history_run_digest: &self.history_run_digest,
            report_digest: &self.report_digest,
            binding: self.binding,
            commit: &self.commit,
            tree: &self.tree,
            parents: &self.parents,
        };

        let actual = compute_record_digest(&payload)?;

        if actual != self.record_digest {
            return Err(GitProvenanceError::RecordDigestMismatch {
                expected: self.record_digest,

                actual,
            });
        }

        Ok(())
    }

    /// Verifies that this record names exactly the supplied immutable history
    /// run.
    pub fn verify_against(&self, run: &DiagnosticHistoryRun) -> Result<(), GitProvenanceError> {
        self.verify_record()?;

        if self.run_index != run.index {
            return Err(GitProvenanceError::RunIndexMismatch {
                record: self.run_index,

                history: run.index,
            });
        }

        let expected_run_digest = run.run_digest.to_string();

        if self.history_run_digest != expected_run_digest {
            return Err(GitProvenanceError::HistoryRunDigestMismatch {
                expected: expected_run_digest,

                actual: self.history_run_digest.clone(),
            });
        }

        if self.report_digest != run.report_digest {
            return Err(GitProvenanceError::ReportDigestMismatch {
                expected: run.report_digest.clone(),

                actual: self.report_digest.clone(),
            });
        }

        Ok(())
    }

    /// Persists this record beside the diagnostic history using create-new
    /// semantics.
    ///
    /// Persisting the exact same already-valid record is idempotent. A
    /// different record for the same history run is rejected.
    pub fn persist(&self, history: &DiagnosticHistory) -> Result<PathBuf, GitProvenanceError> {
        let run = history
            .runs()
            .get(self.run_index)
            .ok_or(GitProvenanceError::RunOutOfRange {
                index: self.run_index,

                len: history.len(),
            })?;

        self.verify_against(run)?;

        let directory = history.directory().join(GIT_PROVENANCE_DIRECTORY);

        fs::create_dir_all(&directory)
            .map_err(|source| io_error("create Git provenance directory", &directory, source))?;

        let path = record_path_for(history, self.run_index);

        if path.exists() {
            let existing = Self::load(history, self.run_index)?
                .ok_or_else(|| GitProvenanceError::AlreadyExists { path: path.clone() })?;

            if &existing == self {
                return Ok(path);
            }

            return Err(GitProvenanceError::AlreadyExists { path });
        }

        let mut bytes = serde_json::to_vec_pretty(self).map_err(GitProvenanceError::Json)?;

        bytes.push(b'\n');

        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,

            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                return Err(GitProvenanceError::AlreadyExists { path });
            }

            Err(source) => {
                return Err(io_error("create Git provenance record", &path, source));
            }
        };

        file.write_all(&bytes)
            .map_err(|source| io_error("write Git provenance record", &path, source))?;

        file.sync_all()
            .map_err(|source| io_error("sync Git provenance record", &path, source))?;

        sync_directory(&directory)?;

        Ok(path)
    }

    /// Loads and verifies the provenance record for one history run.
    pub fn load(
        history: &DiagnosticHistory,
        run_index: usize,
    ) -> Result<Option<Self>, GitProvenanceError> {
        let run = history
            .runs()
            .get(run_index)
            .ok_or(GitProvenanceError::RunOutOfRange {
                index: run_index,

                len: history.len(),
            })?;

        let path = record_path_for(history, run_index);

        if !path.is_file() {
            return Ok(None);
        }

        let bytes = fs::read(&path)
            .map_err(|source| io_error("read Git provenance record", &path, source))?;

        let record: Self = serde_json::from_slice(&bytes).map_err(GitProvenanceError::Json)?;

        record.verify_against(run)?;

        Ok(Some(record))
    }

    pub fn record_path(history: &DiagnosticHistory, run_index: usize) -> PathBuf {
        record_path_for(history, run_index)
    }
}

fn compute_record_digest(
    payload: &GitProvenanceDigestPayload<'_>,
) -> Result<ArtifactDigest, GitProvenanceError> {
    let bytes = serde_json::to_vec(payload).map_err(GitProvenanceError::Json)?;

    Ok(ArtifactDigest::compute(&bytes))
}

fn validate_object_id(field: &'static str, value: &str) -> Result<(), GitProvenanceError> {
    let valid_length = matches!(value.len(), 40 | 64);

    let valid_hex = value.bytes().all(|value| value.is_ascii_hexdigit());

    if !valid_length || !valid_hex {
        return Err(GitProvenanceError::InvalidObjectId {
            field,

            value: value.to_owned(),
        });
    }

    Ok(())
}

fn record_path_for(history: &DiagnosticHistory, run_index: usize) -> PathBuf {
    history
        .directory()
        .join(GIT_PROVENANCE_DIRECTORY)
        .join(format!("run-{run_index:06}.json"))
}

fn sync_directory(path: &Path) -> Result<(), GitProvenanceError> {
    let directory = File::open(path)
        .map_err(|source| io_error("open Git provenance directory for sync", path, source))?;

    directory
        .sync_all()
        .map_err(|source| io_error("sync Git provenance directory", path, source))
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> GitProvenanceError {
    GitProvenanceError::Io {
        operation,

        path: path.to_path_buf(),

        source,
    }
}

#[derive(Debug)]
pub enum GitProvenanceError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },

    Json(serde_json::Error),

    UnsupportedSchema {
        schema: String,
    },

    InvalidObjectId {
        field: &'static str,
        value: String,
    },

    RunOutOfRange {
        index: usize,
        len: usize,
    },

    RunIndexMismatch {
        record: usize,
        history: usize,
    },

    HistoryRunDigestMismatch {
        expected: String,
        actual: String,
    },

    ReportDigestMismatch {
        expected: String,
        actual: String,
    },

    RecordDigestMismatch {
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },

    AlreadyExists {
        path: PathBuf,
    },
}

impl fmt::Display for GitProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => {
                write!(formatter, "{operation} {}: {source}", path.display(),)
            }

            Self::Json(source) => {
                write!(formatter, "Git provenance JSON error: {source}",)
            }

            Self::UnsupportedSchema { schema } => {
                write!(formatter, "unsupported Git provenance schema {schema:?}",)
            }

            Self::InvalidObjectId { field, value } => {
                write!(formatter, "invalid Git {field} object id {value:?}",)
            }

            Self::RunOutOfRange { index, len } => {
                write!(
                    formatter,
                    "Git provenance run index {index} is out of range for history containing {len} run(s)",
                )
            }

            Self::RunIndexMismatch { record, history } => {
                write!(
                    formatter,
                    "Git provenance run index {record} does not match history run {history}",
                )
            }

            Self::HistoryRunDigestMismatch { expected, actual } => {
                write!(
                    formatter,
                    "Git provenance history-run digest mismatch: expected {expected}, got {actual}",
                )
            }

            Self::ReportDigestMismatch { expected, actual } => {
                write!(
                    formatter,
                    "Git provenance report digest mismatch: expected {expected}, got {actual}",
                )
            }

            Self::RecordDigestMismatch { expected, actual } => {
                write!(
                    formatter,
                    "Git provenance record digest mismatch: expected {expected}, got {actual}",
                )
            }

            Self::AlreadyExists { path } => {
                write!(
                    formatter,
                    "Git provenance record already exists with different content: {}",
                    path.display(),
                )
            }
        }
    }
}

impl Error for GitProvenanceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),

            Self::Json(source) => Some(source),

            _ => None,
        }
    }
}
