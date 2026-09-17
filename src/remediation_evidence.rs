use crate::{
    Applicability, ArtifactDigest, DiagnosticHistory, DiagnosticHistoryError, DiagnosticHistoryRun,
    HistoryDeltaCounts, REMEDIATION_RECEIPT_V1_SCHEMA, RemediationReceipt,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const REMEDIATION_EVIDENCE_V1_SCHEMA: &str = "diagprint.remediation.evidence/v1";

const REMEDIATION_DIRECTORY: &str = "remediation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationEvidenceEffect {
    pub before_diagnostics: usize,
    pub after_diagnostics: usize,

    pub new: usize,
    pub resolved: usize,
    pub persisting: usize,
    pub changed: usize,

    pub introduced_errors: usize,
    pub severity_increases: usize,
}

impl RemediationEvidenceEffect {
    fn from_history(
        before: &DiagnosticHistoryRun,
        after: &DiagnosticHistoryRun,
        counts: HistoryDeltaCounts,
        introduced_errors: usize,
        severity_increases: usize,
    ) -> Self {
        Self {
            before_diagnostics: before.diagnostics,
            after_diagnostics: after.diagnostics,

            new: counts.new,
            resolved: counts.resolved,
            persisting: counts.persisting,
            changed: counts.changed,

            introduced_errors,
            severity_increases,
        }
    }

    fn from_receipt(receipt: &RemediationReceipt) -> Self {
        Self {
            before_diagnostics: receipt.effect.before_diagnostics,
            after_diagnostics: receipt.effect.after_diagnostics,

            new: receipt.effect.new,
            resolved: receipt.effect.resolved,
            persisting: receipt.effect.persisting,
            changed: receipt.effect.changed,

            introduced_errors: receipt.effect.introduced_errors,
            severity_increases: receipt.effect.severity_increases,
        }
    }
}

/// Privacy-light evidence binding one exact successful remediation receipt to
/// one exact adjacent transition in a verified diagnostic history.
///
/// The record deliberately excludes source paths, changed-file paths, edit
/// contents, verification payloads, diagnostic messages, and the fix-plan
/// title/explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationEvidenceRecord {
    pub schema: String,

    pub before_run: usize,
    pub after_run: usize,

    pub before_run_digest: ArtifactDigest,
    pub after_run_digest: ArtifactDigest,

    pub before_report_digest: String,
    pub after_report_digest: String,

    pub receipt_schema: String,
    pub receipt_digest: ArtifactDigest,

    pub plan_descriptor_digest: ArtifactDigest,
    pub plan_descriptor_byte_length: usize,
    pub plan_applicability: Applicability,

    pub remediation_status: String,

    pub changed_files: usize,
    pub verification_checks: usize,
    pub verification_passed: bool,

    pub effect: RemediationEvidenceEffect,

    pub record_digest: ArtifactDigest,
}

#[derive(Serialize)]
struct RemediationEvidenceDigestPayload<'a> {
    schema: &'a str,

    before_run: usize,
    after_run: usize,

    before_run_digest: ArtifactDigest,
    after_run_digest: ArtifactDigest,

    before_report_digest: &'a str,
    after_report_digest: &'a str,

    receipt_schema: &'a str,
    receipt_digest: ArtifactDigest,

    plan_descriptor_digest: ArtifactDigest,
    plan_descriptor_byte_length: usize,
    plan_applicability: Applicability,

    remediation_status: &'a str,

    changed_files: usize,
    verification_checks: usize,
    verification_passed: bool,

    effect: RemediationEvidenceEffect,
}

impl RemediationEvidenceRecord {
    fn from_verified_transition(
        before: &DiagnosticHistoryRun,
        after: &DiagnosticHistoryRun,
        receipt: &RemediationReceipt,
        effect: RemediationEvidenceEffect,
        receipt_digest: ArtifactDigest,
    ) -> Result<Self, RemediationEvidenceError> {
        let mut record = Self {
            schema: REMEDIATION_EVIDENCE_V1_SCHEMA.to_owned(),

            before_run: before.index,
            after_run: after.index,

            before_run_digest: before.run_digest,
            after_run_digest: after.run_digest,

            before_report_digest: before.report_digest.clone(),
            after_report_digest: after.report_digest.clone(),

            receipt_schema: receipt.schema.to_owned(),
            receipt_digest,

            plan_descriptor_digest: receipt.plan.descriptor_digest,
            plan_descriptor_byte_length: receipt.plan.descriptor_byte_length,
            plan_applicability: receipt.plan.applicability,

            remediation_status: receipt.status().as_str().to_owned(),

            changed_files: receipt.outcome.changed_files,
            verification_checks: receipt.outcome.verification_checks,
            verification_passed: receipt.outcome.verification_passed,

            effect,

            record_digest: ArtifactDigest::compute(&[]),
        };

        record.record_digest = record.recompute_digest()?;
        Ok(record)
    }

    fn recompute_digest(&self) -> Result<ArtifactDigest, RemediationEvidenceError> {
        let payload = RemediationEvidenceDigestPayload {
            schema: &self.schema,

            before_run: self.before_run,
            after_run: self.after_run,

            before_run_digest: self.before_run_digest,
            after_run_digest: self.after_run_digest,

            before_report_digest: &self.before_report_digest,
            after_report_digest: &self.after_report_digest,

            receipt_schema: &self.receipt_schema,
            receipt_digest: self.receipt_digest,

            plan_descriptor_digest: self.plan_descriptor_digest,
            plan_descriptor_byte_length: self.plan_descriptor_byte_length,
            plan_applicability: self.plan_applicability,

            remediation_status: &self.remediation_status,

            changed_files: self.changed_files,
            verification_checks: self.verification_checks,
            verification_passed: self.verification_passed,

            effect: self.effect,
        };

        let bytes = serde_json::to_vec(&payload).map_err(RemediationEvidenceError::Json)?;

        Ok(ArtifactDigest::compute(&bytes))
    }
}

impl DiagnosticHistory {
    /// Binds one exact successful remediation receipt to an exact adjacent
    /// transition in this verified history.
    ///
    /// The history itself is not modified. Evidence is persisted as a separate
    /// append-only sidecar under `<history>/remediation/`.
    pub fn record_remediation_evidence(
        &self,
        before_run: usize,
        after_run: usize,
        receipt: &RemediationReceipt,
    ) -> Result<RemediationEvidenceRecord, RemediationEvidenceError> {
        self.verify().map_err(RemediationEvidenceError::History)?;

        // Refuse to append new evidence beside already-invalid sidecars.
        self.verify_remediation_evidence()?;

        if after_run != before_run.saturating_add(1) {
            return Err(RemediationEvidenceError::NonAdjacentTransition {
                before_run,
                after_run,
            });
        }

        let before =
            self.runs()
                .get(before_run)
                .ok_or(RemediationEvidenceError::RunOutOfRange {
                    index: before_run,
                    len: self.len(),
                })?;

        let after = self
            .runs()
            .get(after_run)
            .ok_or(RemediationEvidenceError::RunOutOfRange {
                index: after_run,
                len: self.len(),
            })?;

        if receipt.schema != REMEDIATION_RECEIPT_V1_SCHEMA {
            return Err(RemediationEvidenceError::UnsupportedReceiptSchema {
                schema: receipt.schema.to_owned(),
            });
        }

        let receipt_before = receipt.before_report.qualified();
        let receipt_after = receipt.after_report.qualified();

        if receipt_before != before.report_digest {
            return Err(RemediationEvidenceError::ReceiptBeforeReportMismatch {
                expected: before.report_digest.clone(),
                actual: receipt_before,
            });
        }

        if receipt_after != after.report_digest {
            return Err(RemediationEvidenceError::ReceiptAfterReportMismatch {
                expected: after.report_digest.clone(),
                actual: receipt_after,
            });
        }

        let transition = self
            .transition(before_run, after_run)
            .map_err(RemediationEvidenceError::History)?;

        let expected_effect = RemediationEvidenceEffect::from_history(
            before,
            after,
            transition.counts,
            transition.introduced_errors,
            transition.severity_increases,
        );

        let actual_effect = RemediationEvidenceEffect::from_receipt(receipt);

        if actual_effect != expected_effect {
            return Err(RemediationEvidenceError::EffectMismatch {
                expected: Box::new(expected_effect),
                actual: Box::new(actual_effect),
            });
        }

        if !receipt.outcome.verification_passed {
            return Err(RemediationEvidenceError::ReceiptVerificationNotPassed);
        }

        let receipt_bytes = serde_json::to_vec(receipt).map_err(RemediationEvidenceError::Json)?;
        let receipt_digest = ArtifactDigest::compute(&receipt_bytes);

        let record = RemediationEvidenceRecord::from_verified_transition(
            before,
            after,
            receipt,
            expected_effect,
            receipt_digest,
        )?;

        persist_record(self, &record)?;
        Ok(record)
    }

    /// Loads and verifies the remediation evidence associated with one
    /// post-remediation history run.
    pub fn remediation_evidence(
        &self,
        after_run: usize,
    ) -> Result<Option<RemediationEvidenceRecord>, RemediationEvidenceError> {
        self.verify().map_err(RemediationEvidenceError::History)?;

        let path = remediation_path(self.directory(), after_run);

        if !path
            .try_exists()
            .map_err(|source| io_error("check remediation evidence", &path, source))?
        {
            return Ok(None);
        }

        load_record(self, after_run, &path).map(Some)
    }

    /// Loads every persisted remediation evidence record in deterministic
    /// post-remediation run order and verifies each record against history.
    pub fn remediation_evidence_records(
        &self,
    ) -> Result<Vec<RemediationEvidenceRecord>, RemediationEvidenceError> {
        self.verify().map_err(RemediationEvidenceError::History)?;

        let directory = self.directory().join(REMEDIATION_DIRECTORY);

        if !directory
            .try_exists()
            .map_err(|source| io_error("check remediation directory", &directory, source))?
        {
            return Ok(Vec::new());
        }

        let mut discovered = Vec::new();

        for entry in fs::read_dir(&directory)
            .map_err(|source| io_error("read remediation directory", &directory, source))?
        {
            let entry = entry.map_err(|source| {
                io_error("read remediation directory entry", &directory, source)
            })?;

            if !entry
                .file_type()
                .map_err(|source| {
                    io_error(
                        "read remediation evidence entry type",
                        &entry.path(),
                        source,
                    )
                })?
                .is_file()
            {
                continue;
            }

            let path = entry.path();
            let Some(index) = parse_evidence_filename(
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default(),
            ) else {
                continue;
            };

            discovered.push((index, path));
        }

        discovered.sort_by_key(|(index, _)| *index);

        let mut records = Vec::with_capacity(discovered.len());

        for (index, path) in discovered {
            records.push(load_record(self, index, &path)?);
        }

        Ok(records)
    }

    /// Re-verifies the complete history chain and every remediation evidence
    /// sidecar currently present.
    pub fn verify_remediation_evidence(&self) -> Result<(), RemediationEvidenceError> {
        let _ = self.remediation_evidence_records()?;
        Ok(())
    }
}

fn persist_record(
    history: &DiagnosticHistory,
    record: &RemediationEvidenceRecord,
) -> Result<(), RemediationEvidenceError> {
    let directory = history.directory().join(REMEDIATION_DIRECTORY);
    fs::create_dir_all(&directory)
        .map_err(|source| io_error("create remediation directory", &directory, source))?;

    let path = remediation_path(history.directory(), record.after_run);

    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            let bytes =
                serde_json::to_vec_pretty(record).map_err(RemediationEvidenceError::Json)?;

            if let Err(source) = write_and_sync(&mut file, &bytes) {
                let _ = fs::remove_file(&path);
                return Err(io_error("write remediation evidence", &path, source));
            }

            Ok(())
        }

        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
            let existing = load_record(history, record.after_run, &path)?;

            if existing == *record {
                Ok(())
            } else {
                Err(RemediationEvidenceError::ConflictingRecord {
                    after_run: record.after_run,
                    path,
                })
            }
        }

        Err(source) => Err(io_error("create remediation evidence", &path, source)),
    }
}

fn write_and_sync(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

fn load_record(
    history: &DiagnosticHistory,
    file_index: usize,
    path: &Path,
) -> Result<RemediationEvidenceRecord, RemediationEvidenceError> {
    let bytes =
        fs::read(path).map_err(|source| io_error("read remediation evidence", path, source))?;

    let record: RemediationEvidenceRecord =
        serde_json::from_slice(&bytes).map_err(RemediationEvidenceError::Json)?;

    verify_record(history, file_index, path, &record)?;
    Ok(record)
}

fn verify_record(
    history: &DiagnosticHistory,
    file_index: usize,
    path: &Path,
    record: &RemediationEvidenceRecord,
) -> Result<(), RemediationEvidenceError> {
    if record.schema != REMEDIATION_EVIDENCE_V1_SCHEMA {
        return Err(RemediationEvidenceError::UnsupportedSchema {
            path: path.to_path_buf(),
            schema: record.schema.clone(),
        });
    }

    if record.after_run != file_index {
        return Err(RemediationEvidenceError::FileIndexMismatch {
            path: path.to_path_buf(),
            file_index,
            record_index: record.after_run,
        });
    }

    if record.after_run != record.before_run.saturating_add(1) {
        return Err(RemediationEvidenceError::NonAdjacentTransition {
            before_run: record.before_run,
            after_run: record.after_run,
        });
    }

    if record.receipt_schema != REMEDIATION_RECEIPT_V1_SCHEMA {
        return Err(RemediationEvidenceError::UnsupportedReceiptSchema {
            schema: record.receipt_schema.clone(),
        });
    }

    if !matches!(record.remediation_status.as_str(), "applied" | "verified") {
        return Err(RemediationEvidenceError::InvalidRemediationStatus {
            status: record.remediation_status.clone(),
        });
    }

    if !record.verification_passed {
        return Err(RemediationEvidenceError::ReceiptVerificationNotPassed);
    }

    let actual_digest = record.recompute_digest()?;

    if actual_digest != record.record_digest {
        return Err(RemediationEvidenceError::RecordDigestMismatch {
            path: path.to_path_buf(),
            expected: record.record_digest,
            actual: actual_digest,
        });
    }

    let before =
        history
            .runs()
            .get(record.before_run)
            .ok_or(RemediationEvidenceError::RunOutOfRange {
                index: record.before_run,
                len: history.len(),
            })?;

    let after =
        history
            .runs()
            .get(record.after_run)
            .ok_or(RemediationEvidenceError::RunOutOfRange {
                index: record.after_run,
                len: history.len(),
            })?;

    if record.before_run_digest != before.run_digest {
        return Err(RemediationEvidenceError::BeforeRunDigestMismatch {
            run: record.before_run,
            expected: before.run_digest,
            actual: record.before_run_digest,
        });
    }

    if record.after_run_digest != after.run_digest {
        return Err(RemediationEvidenceError::AfterRunDigestMismatch {
            run: record.after_run,
            expected: after.run_digest,
            actual: record.after_run_digest,
        });
    }

    if record.before_report_digest != before.report_digest {
        return Err(RemediationEvidenceError::BeforeReportDigestMismatch {
            run: record.before_run,
            expected: before.report_digest.clone(),
            actual: record.before_report_digest.clone(),
        });
    }

    if record.after_report_digest != after.report_digest {
        return Err(RemediationEvidenceError::AfterReportDigestMismatch {
            run: record.after_run,
            expected: after.report_digest.clone(),
            actual: record.after_report_digest.clone(),
        });
    }

    let transition = history
        .transition(record.before_run, record.after_run)
        .map_err(RemediationEvidenceError::History)?;

    let expected_effect = RemediationEvidenceEffect::from_history(
        before,
        after,
        transition.counts,
        transition.introduced_errors,
        transition.severity_increases,
    );

    if record.effect != expected_effect {
        return Err(RemediationEvidenceError::EffectMismatch {
            expected: Box::new(expected_effect),
            actual: Box::new(record.effect),
        });
    }

    Ok(())
}

fn remediation_path(history_directory: &Path, after_run: usize) -> PathBuf {
    history_directory
        .join(REMEDIATION_DIRECTORY)
        .join(evidence_filename(after_run))
}

fn evidence_filename(index: usize) -> String {
    format!("run-{index:06}.json")
}

fn parse_evidence_filename(name: &str) -> Option<usize> {
    let value = name.strip_prefix("run-")?.strip_suffix(".json")?;

    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    value.parse().ok()
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> RemediationEvidenceError {
    RemediationEvidenceError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug)]
pub enum RemediationEvidenceError {
    History(DiagnosticHistoryError),
    Json(serde_json::Error),

    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },

    RunOutOfRange {
        index: usize,
        len: usize,
    },

    NonAdjacentTransition {
        before_run: usize,
        after_run: usize,
    },

    UnsupportedSchema {
        path: PathBuf,
        schema: String,
    },

    UnsupportedReceiptSchema {
        schema: String,
    },

    InvalidRemediationStatus {
        status: String,
    },

    ReceiptBeforeReportMismatch {
        expected: String,
        actual: String,
    },

    ReceiptAfterReportMismatch {
        expected: String,
        actual: String,
    },

    ReceiptVerificationNotPassed,

    EffectMismatch {
        expected: Box<RemediationEvidenceEffect>,
        actual: Box<RemediationEvidenceEffect>,
    },

    FileIndexMismatch {
        path: PathBuf,
        file_index: usize,
        record_index: usize,
    },

    BeforeRunDigestMismatch {
        run: usize,
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },

    AfterRunDigestMismatch {
        run: usize,
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },

    BeforeReportDigestMismatch {
        run: usize,
        expected: String,
        actual: String,
    },

    AfterReportDigestMismatch {
        run: usize,
        expected: String,
        actual: String,
    },

    RecordDigestMismatch {
        path: PathBuf,
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },

    ConflictingRecord {
        after_run: usize,
        path: PathBuf,
    },
}

impl fmt::Display for RemediationEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::History(source) => {
                write!(
                    formatter,
                    "diagnostic history verification failed: {source}"
                )
            }

            Self::Json(source) => {
                write!(formatter, "remediation evidence JSON failed: {source}")
            }

            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display()
            ),

            Self::RunOutOfRange { index, len } => write!(
                formatter,
                "remediation evidence run {index} is out of range for history length {len}"
            ),

            Self::NonAdjacentTransition {
                before_run,
                after_run,
            } => write!(
                formatter,
                "remediation evidence requires adjacent history runs, got {before_run} -> {after_run}"
            ),

            Self::UnsupportedSchema { path, schema } => write!(
                formatter,
                "unsupported remediation evidence schema {schema:?} in {}",
                path.display()
            ),

            Self::UnsupportedReceiptSchema { schema } => write!(
                formatter,
                "unsupported remediation receipt schema {schema:?}"
            ),

            Self::InvalidRemediationStatus { status } => {
                write!(formatter, "invalid remediation evidence status {status:?}")
            }

            Self::ReceiptBeforeReportMismatch { expected, actual } => write!(
                formatter,
                "remediation receipt before-report mismatch: expected {expected}, got {actual}"
            ),

            Self::ReceiptAfterReportMismatch { expected, actual } => write!(
                formatter,
                "remediation receipt after-report mismatch: expected {expected}, got {actual}"
            ),

            Self::ReceiptVerificationNotPassed => formatter
                .write_str("remediation evidence requires a receipt with verification_passed=true"),

            Self::EffectMismatch { expected, actual } => write!(
                formatter,
                "remediation receipt effect does not match history transition: expected {expected:?}, got {actual:?}"
            ),

            Self::FileIndexMismatch {
                path,
                file_index,
                record_index,
            } => write!(
                formatter,
                "remediation evidence filename/index mismatch in {}: filename run {file_index}, record run {record_index}",
                path.display()
            ),

            Self::BeforeRunDigestMismatch {
                run,
                expected,
                actual,
            } => write!(
                formatter,
                "remediation evidence before-run digest mismatch for run {run}: expected {expected}, got {actual}"
            ),

            Self::AfterRunDigestMismatch {
                run,
                expected,
                actual,
            } => write!(
                formatter,
                "remediation evidence after-run digest mismatch for run {run}: expected {expected}, got {actual}"
            ),

            Self::BeforeReportDigestMismatch {
                run,
                expected,
                actual,
            } => write!(
                formatter,
                "remediation evidence before-report mismatch for run {run}: expected {expected}, got {actual}"
            ),

            Self::AfterReportDigestMismatch {
                run,
                expected,
                actual,
            } => write!(
                formatter,
                "remediation evidence after-report mismatch for run {run}: expected {expected}, got {actual}"
            ),

            Self::RecordDigestMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "remediation evidence digest mismatch in {}: expected {expected}, got {actual}",
                path.display()
            ),

            Self::ConflictingRecord { after_run, path } => write!(
                formatter,
                "conflicting remediation evidence already exists for post-remediation run {after_run} at {}",
                path.display()
            ),
        }
    }
}

impl Error for RemediationEvidenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::History(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::Io { source, .. } => Some(source),

            _ => None,
        }
    }
}

impl From<DiagnosticHistoryError> for RemediationEvidenceError {
    fn from(source: DiagnosticHistoryError) -> Self {
        Self::History(source)
    }
}
