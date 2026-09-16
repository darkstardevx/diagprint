use crate::{
    ArtifactDigest, ArtifactVerificationError, CanonicalizationError, DiagnosticReport,
    ExportPolicy, FixPlan, RemediationReceipt, ReportDigest, SourceSnapshot,
    render::{JsonRenderer, RenderedArtifact, RenderedFormat},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    ffi::OsStr,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

/// Stable schema identifier for a diagnostic capsule manifest.
pub const DIAGNOSTIC_CAPSULE_V1_SCHEMA: &str = "diagprint.capsule/v1";

/// Stable schema for capsule provenance metadata.
pub const CAPSULE_PROVENANCE_V1_SCHEMA: &str = "diagprint.capsule.provenance/v1";

/// Stable schema for an exported source snapshot index.
pub const CAPSULE_SOURCE_INDEX_V1_SCHEMA: &str = "diagprint.capsule.sources/v1";

/// Controls privacy-sensitive capsule contents.
///
/// Source snapshots and full remediation descriptors are excluded by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticCapsulePolicy {
    include_sources: bool,
    include_remediation_plan: bool,
}

impl DiagnosticCapsulePolicy {
    pub const fn new() -> Self {
        Self {
            include_sources: false,
            include_remediation_plan: false,
        }
    }

    pub const fn with_sources(mut self, enabled: bool) -> Self {
        self.include_sources = enabled;
        self
    }

    pub const fn with_remediation_plan(mut self, enabled: bool) -> Self {
        self.include_remediation_plan = enabled;
        self
    }

    pub const fn includes_sources(self) -> bool {
        self.include_sources
    }

    pub const fn includes_remediation_plan(self) -> bool {
        self.include_remediation_plan
    }

    const fn descriptor(self) -> CapsulePolicyDescriptor {
        CapsulePolicyDescriptor {
            include_sources: self.include_sources,

            include_remediation_plan: self.include_remediation_plan,
        }
    }
}

/// Privacy-safe capsule-policy description stored in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsulePolicyDescriptor {
    pub include_sources: bool,
    pub include_remediation_plan: bool,
}

/// Logical role of one capsule payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapsuleEntryKind {
    DiagnosticReport,
    RenderedReport,
    RenderedReceipt,
    RemediationReceipt,
    RemediationPlan,
    Provenance,
    SourceIndex,
    Source,
}

/// One exact payload recorded by the capsule manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsuleManifestEntry {
    pub path: String,
    pub kind: CapsuleEntryKind,
    pub media_type: String,
    pub artifact_digest: ArtifactDigest,
    pub byte_length: usize,
}

/// Manifest for one diagnostic capsule.
///
/// The manifest deliberately does not contain its own digest. Persisted and
/// verified capsules expose the exact manifest digest separately, avoiding a
/// recursive self-hash definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCapsuleManifest {
    pub schema: String,

    pub report_digest: String,

    pub policy: CapsulePolicyDescriptor,

    pub entries: Vec<CapsuleManifestEntry>,
}

/// Deterministic producer/project provenance.
///
/// Attributes are caller-controlled because values such as Git revisions,
/// compiler versions, target triples, and CI identifiers have different
/// privacy implications in different environments.
#[derive(Debug, Clone, Serialize)]
pub struct CapsuleProvenance {
    pub schema: &'static str,

    pub producer: &'static str,
    pub producer_version: &'static str,

    pub attributes: BTreeMap<String, String>,
}

impl Default for CapsuleProvenance {
    fn default() -> Self {
        Self::new()
    }
}

impl CapsuleProvenance {
    pub fn new() -> Self {
        Self {
            schema: CAPSULE_PROVENANCE_V1_SCHEMA,
            producer: "diagprint",
            producer_version: env!("CARGO_PKG_VERSION"),
            attributes: BTreeMap::new(),
        }
    }

    pub fn attribute(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(name.into(), value.into());

        self
    }
}

/// One exported source in a capsule source index.
#[derive(Debug, Clone, Serialize)]
pub struct CapsuleSourceEntry {
    pub name: String,
    pub revision: u64,

    pub path: String,

    pub artifact_digest: ArtifactDigest,
    pub byte_length: usize,
}

/// Index describing an explicitly exported source snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct CapsuleSourceIndex {
    pub schema: &'static str,

    pub sources: Vec<CapsuleSourceEntry>,
}

#[derive(Debug, Clone)]
struct CapsulePayload {
    kind: CapsuleEntryKind,
    media_type: String,
    bytes: Vec<u8>,
}

impl CapsulePayload {
    fn manifest_entry(&self, path: &str) -> CapsuleManifestEntry {
        CapsuleManifestEntry {
            path: path.to_owned(),

            kind: self.kind,

            media_type: self.media_type.clone(),

            artifact_digest: ArtifactDigest::compute(&self.bytes),

            byte_length: self.bytes.len(),
        }
    }
}

/// In-memory diagnostic capsule.
///
/// A capsule is anchored to one semantic [`ReportDigest`]. Every payload added
/// to it is exact-byte hashed in the manifest.
#[derive(Debug, Clone)]
pub struct DiagnosticCapsule {
    report_digest: ReportDigest,

    policy: DiagnosticCapsulePolicy,

    entries: BTreeMap<String, CapsulePayload>,
}

impl DiagnosticCapsule {
    /// Builds a capsule using diagprint's default external export policy.
    pub fn new(report: &DiagnosticReport) -> Result<Self, DiagnosticCapsuleError> {
        Self::with_policies(
            report,
            &ExportPolicy::default(),
            DiagnosticCapsulePolicy::default(),
        )
    }

    /// Builds a capsule with explicit export and capsule privacy policies.
    pub fn with_policies(
        report: &DiagnosticReport,
        export_policy: &ExportPolicy,
        capsule_policy: DiagnosticCapsulePolicy,
    ) -> Result<Self, DiagnosticCapsuleError> {
        let report_digest = report
            .digest()
            .map_err(DiagnosticCapsuleError::Canonicalization)?;

        let report_json = JsonRenderer
            .try_render_report_with_policy(report.iter(), export_policy)
            .map_err(DiagnosticCapsuleError::Json)?;

        let mut capsule = Self {
            report_digest,
            policy: capsule_policy,
            entries: BTreeMap::new(),
        };

        capsule.insert_entry(
            "reports/report.json",
            CapsuleEntryKind::DiagnosticReport,
            "application/json",
            report_json.into_bytes(),
        )?;

        Ok(capsule)
    }

    pub const fn report_digest(&self) -> ReportDigest {
        self.report_digest
    }

    pub const fn policy(&self) -> DiagnosticCapsulePolicy {
        self.policy
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns a deterministic manifest for the current in-memory capsule.
    pub fn manifest(&self) -> DiagnosticCapsuleManifest {
        DiagnosticCapsuleManifest {
            schema: DIAGNOSTIC_CAPSULE_V1_SCHEMA.to_owned(),

            report_digest: self.report_digest.qualified(),

            policy: self.policy.descriptor(),

            entries: self
                .entries
                .iter()
                .map(|(path, payload)| payload.manifest_entry(path))
                .collect(),
        }
    }

    /// Adds one verified rendered representation of this capsule's report.
    ///
    /// The rendered artifact and its receipt are stored as separate,
    /// independently hashed capsule entries.
    pub fn add_rendered(
        &mut self,
        artifact: &RenderedArtifact,
    ) -> Result<&mut Self, DiagnosticCapsuleError> {
        artifact
            .verify()
            .map_err(DiagnosticCapsuleError::ArtifactVerification)?;

        if artifact.receipt().report != self.report_digest {
            return Err(DiagnosticCapsuleError::ReportMismatch {
                capsule: self.report_digest.qualified(),

                supplied: artifact.receipt().report.qualified(),
            });
        }

        let filename = rendered_filename(artifact.format());

        let report_path = format!("reports/{filename}");

        self.insert_entry(
            &report_path,
            CapsuleEntryKind::RenderedReport,
            artifact.receipt().media_type,
            artifact.bytes().to_vec(),
        )?;

        let receipt_bytes =
            serde_json::to_vec_pretty(artifact.receipt()).map_err(DiagnosticCapsuleError::Json)?;

        let receipt_path = format!("receipts/rendered/{filename}.receipt.json");

        self.insert_entry(
            &receipt_path,
            CapsuleEntryKind::RenderedReceipt,
            "application/json",
            receipt_bytes,
        )?;

        Ok(self)
    }

    /// Adds producer/project provenance.
    pub fn add_provenance(
        &mut self,
        provenance: &CapsuleProvenance,
    ) -> Result<&mut Self, DiagnosticCapsuleError> {
        let bytes = serde_json::to_vec_pretty(provenance).map_err(DiagnosticCapsuleError::Json)?;

        self.insert_entry(
            "provenance/project.json",
            CapsuleEntryKind::Provenance,
            "application/json",
            bytes,
        )?;

        Ok(self)
    }

    /// Adds a remediation receipt and, when explicitly allowed, its exact fix
    /// plan descriptor.
    ///
    /// The capsule report must be either the receipt's before-report or
    /// after-report state.
    pub fn add_remediation(
        &mut self,
        receipt: &RemediationReceipt,
        plan: Option<&FixPlan>,
    ) -> Result<&mut Self, DiagnosticCapsuleError> {
        if receipt.before_report != self.report_digest && receipt.after_report != self.report_digest
        {
            return Err(DiagnosticCapsuleError::RemediationReportMismatch {
                capsule: self.report_digest.qualified(),

                before: receipt.before_report.qualified(),

                after: receipt.after_report.qualified(),
            });
        }

        let receipt_bytes =
            serde_json::to_vec_pretty(receipt).map_err(DiagnosticCapsuleError::Json)?;

        self.insert_entry(
            "receipts/remediation.json",
            CapsuleEntryKind::RemediationReceipt,
            "application/json",
            receipt_bytes,
        )?;

        let Some(plan) = plan else {
            return Ok(self);
        };

        if !self.policy.includes_remediation_plan() {
            return Err(DiagnosticCapsuleError::PolicyDenied {
                payload: "remediation plan descriptor",
            });
        }

        let descriptor = plan.descriptor();

        // RemediationReceipt hashes the compact JSON form. Store those same
        // exact bytes so the capsule can prove that this descriptor is the
        // descriptor named by the receipt.
        let descriptor_bytes =
            serde_json::to_vec(&descriptor).map_err(DiagnosticCapsuleError::Json)?;

        let actual_digest = ArtifactDigest::compute(&descriptor_bytes);

        let actual_length = descriptor_bytes.len();

        if actual_digest != receipt.plan.descriptor_digest
            || actual_length != receipt.plan.descriptor_byte_length
        {
            return Err(DiagnosticCapsuleError::RemediationPlanMismatch {
                expected_digest: receipt.plan.descriptor_digest,

                actual_digest,

                expected_length: receipt.plan.descriptor_byte_length,

                actual_length,
            });
        }

        self.insert_entry(
            "remediation/plan.json",
            CapsuleEntryKind::RemediationPlan,
            "application/json",
            descriptor_bytes,
        )?;

        Ok(self)
    }

    /// Explicitly adds an immutable source snapshot.
    ///
    /// Source export is denied unless the capsule policy was constructed with
    /// `with_sources(true)`.
    ///
    /// Original source names are retained in `sources/index.json`, but are not
    /// used as filesystem paths inside the capsule. This prevents traversal or
    /// absolute-path source names from escaping the capsule directory.
    pub fn add_sources(
        &mut self,
        snapshot: &SourceSnapshot,
    ) -> Result<&mut Self, DiagnosticCapsuleError> {
        if !self.policy.includes_sources() {
            return Err(DiagnosticCapsuleError::PolicyDenied {
                payload: "source snapshot",
            });
        }

        let mut source_entries = Vec::with_capacity(snapshot.len());

        for (index, name) in snapshot.names().into_iter().enumerate() {
            let Some(entry) = snapshot.entry(&name) else {
                continue;
            };

            let bytes = entry.text().as_bytes().to_vec();

            let source_path = format!("sources/{:06}.txt", index + 1,);

            let digest = ArtifactDigest::compute(&bytes);

            source_entries.push(CapsuleSourceEntry {
                name,

                revision: entry.revision().get(),

                path: source_path.clone(),

                artifact_digest: digest,

                byte_length: bytes.len(),
            });

            self.insert_entry(
                &source_path,
                CapsuleEntryKind::Source,
                "text/plain; charset=utf-8",
                bytes,
            )?;
        }

        let index = CapsuleSourceIndex {
            schema: CAPSULE_SOURCE_INDEX_V1_SCHEMA,

            sources: source_entries,
        };

        let index_bytes =
            serde_json::to_vec_pretty(&index).map_err(DiagnosticCapsuleError::Json)?;

        self.insert_entry(
            "sources/index.json",
            CapsuleEntryKind::SourceIndex,
            "application/json",
            index_bytes,
        )?;

        Ok(self)
    }

    /// Persists the capsule as one create-only transactional directory.
    pub fn write_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<PersistedDiagnosticCapsule, DiagnosticCapsuleError> {
        let destination = destination.as_ref();

        validate_destination(destination)?;

        if destination.exists() {
            return Err(DiagnosticCapsuleError::DestinationExists {
                path: destination.to_path_buf(),
            });
        }

        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        fs::create_dir_all(parent)
            .map_err(|source| io_error("create capsule parent directory", parent, source))?;

        let staging_path = parent.join(format!(".diagprint-capsule-stage-{}", Uuid::now_v7(),));

        fs::create_dir(&staging_path).map_err(|source| {
            io_error("create capsule staging directory", &staging_path, source)
        })?;

        let mut staging = CapsuleStagingDirectory::new(staging_path);

        for (relative, payload) in &self.entries {
            validate_entry_path(relative)?;

            let target = staging.path().join(relative);

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|source| io_error("create capsule entry directory", parent, source))?;
            }

            write_synced(&target, &payload.bytes)?;
        }

        let manifest = self.manifest();

        let manifest_bytes =
            serde_json::to_vec_pretty(&manifest).map_err(DiagnosticCapsuleError::Json)?;

        let manifest_path = staging.path().join("manifest.json");

        write_synced(&manifest_path, &manifest_bytes)?;

        sync_directory_tree(staging.path())?;

        // Verify the exact staged bytes before making the directory visible.
        let verification = Self::verify_directory(staging.path())?;

        if destination.exists() {
            return Err(DiagnosticCapsuleError::DestinationExists {
                path: destination.to_path_buf(),
            });
        }

        fs::rename(staging.path(), destination).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                DiagnosticCapsuleError::DestinationExists {
                    path: destination.to_path_buf(),
                }
            } else {
                io_error("commit diagnostic capsule", destination, source)
            }
        })?;

        staging.mark_committed();

        sync_directory(parent)?;

        Ok(PersistedDiagnosticCapsule {
            directory: destination.to_path_buf(),

            manifest_path: destination.join("manifest.json"),

            manifest_digest: verification.manifest_digest,

            entries: verification.entries,
        })
    }

    /// Verifies every manifest-listed payload in an existing capsule
    /// directory.
    pub fn verify_directory(
        directory: impl AsRef<Path>,
    ) -> Result<DiagnosticCapsuleVerification, DiagnosticCapsuleError> {
        let directory = directory.as_ref();

        let manifest_path = directory.join("manifest.json");

        let manifest_bytes = fs::read(&manifest_path)
            .map_err(|source| io_error("read capsule manifest", &manifest_path, source))?;

        let manifest: DiagnosticCapsuleManifest =
            serde_json::from_slice(&manifest_bytes).map_err(DiagnosticCapsuleError::Json)?;

        if manifest.schema != DIAGNOSTIC_CAPSULE_V1_SCHEMA {
            return Err(DiagnosticCapsuleError::UnsupportedManifestSchema {
                schema: manifest.schema,
            });
        }

        let mut seen = BTreeSet::new();

        for entry in &manifest.entries {
            validate_entry_path(&entry.path)?;

            if entry.path == "manifest.json" {
                return Err(DiagnosticCapsuleError::InvalidEntryPath {
                    path: entry.path.clone(),
                });
            }

            if !seen.insert(entry.path.clone()) {
                return Err(DiagnosticCapsuleError::DuplicateManifestEntry {
                    path: entry.path.clone(),
                });
            }

            let path = directory.join(&entry.path);

            let bytes =
                fs::read(&path).map_err(|source| io_error("read capsule entry", &path, source))?;

            if bytes.len() != entry.byte_length {
                return Err(DiagnosticCapsuleError::EntryLength {
                    path: entry.path.clone(),

                    expected: entry.byte_length,

                    actual: bytes.len(),
                });
            }

            let actual = ArtifactDigest::compute(&bytes);

            if actual != entry.artifact_digest {
                return Err(DiagnosticCapsuleError::EntryDigest {
                    path: entry.path.clone(),

                    expected: entry.artifact_digest,

                    actual,
                });
            }
        }

        Ok(DiagnosticCapsuleVerification {
            report_digest: manifest.report_digest,

            manifest_digest: ArtifactDigest::compute(&manifest_bytes),

            entries: manifest.entries.len(),
        })
    }

    fn insert_entry(
        &mut self,
        path: &str,
        kind: CapsuleEntryKind,
        media_type: &str,
        bytes: Vec<u8>,
    ) -> Result<(), DiagnosticCapsuleError> {
        validate_entry_path(path)?;

        if self.entries.contains_key(path) {
            return Err(DiagnosticCapsuleError::DuplicateEntry {
                path: path.to_owned(),
            });
        }

        self.entries.insert(
            path.to_owned(),
            CapsulePayload {
                kind,
                media_type: media_type.to_owned(),
                bytes,
            },
        );

        Ok(())
    }
}

/// Successful capsule verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticCapsuleVerification {
    pub report_digest: String,

    pub manifest_digest: ArtifactDigest,

    pub entries: usize,
}

/// Result of one committed capsule transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedDiagnosticCapsule {
    directory: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: ArtifactDigest,
    entries: usize,
}

impl PersistedDiagnosticCapsule {
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub const fn manifest_digest(&self) -> ArtifactDigest {
        self.manifest_digest
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }
}

/// Capsule construction, persistence, or verification failure.
#[derive(Debug)]
pub enum DiagnosticCapsuleError {
    Canonicalization(CanonicalizationError),

    Json(serde_json::Error),

    ArtifactVerification(ArtifactVerificationError),

    ReportMismatch {
        capsule: String,
        supplied: String,
    },

    RemediationReportMismatch {
        capsule: String,
        before: String,
        after: String,
    },

    RemediationPlanMismatch {
        expected_digest: ArtifactDigest,

        actual_digest: ArtifactDigest,

        expected_length: usize,
        actual_length: usize,
    },

    PolicyDenied {
        payload: &'static str,
    },

    DuplicateEntry {
        path: String,
    },

    DuplicateManifestEntry {
        path: String,
    },

    InvalidEntryPath {
        path: String,
    },

    UnsupportedManifestSchema {
        schema: String,
    },

    DestinationExists {
        path: PathBuf,
    },

    EntryLength {
        path: String,
        expected: usize,
        actual: usize,
    },

    EntryDigest {
        path: String,
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },

    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for DiagnosticCapsuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonicalization(error) => {
                write!(formatter, "could not canonicalize capsule report: {error}")
            }

            Self::Json(error) => {
                write!(formatter, "capsule JSON operation failed: {error}")
            }

            Self::ArtifactVerification(error) => {
                write!(formatter, "rendered artifact failed verification: {error}")
            }

            Self::ReportMismatch { capsule, supplied } => write!(
                formatter,
                "rendered artifact belongs to report {supplied}, but capsule is anchored to {capsule}",
            ),

            Self::RemediationReportMismatch {
                capsule,
                before,
                after,
            } => write!(
                formatter,
                "remediation receipt connects {before} -> {after}, but capsule is anchored to unrelated report {capsule}",
            ),

            Self::RemediationPlanMismatch {
                expected_digest,
                actual_digest,
                expected_length,
                actual_length,
            } => write!(
                formatter,
                "remediation plan descriptor mismatch: expected {expected_digest} ({expected_length} bytes), got {actual_digest} ({actual_length} bytes)",
            ),

            Self::PolicyDenied { payload } => write!(
                formatter,
                "capsule policy does not allow exporting {payload}",
            ),

            Self::DuplicateEntry { path } => {
                write!(formatter, "capsule already contains entry {path:?}",)
            }

            Self::DuplicateManifestEntry { path } => write!(
                formatter,
                "capsule manifest contains duplicate entry {path:?}",
            ),

            Self::InvalidEntryPath { path } => {
                write!(formatter, "invalid capsule-relative entry path {path:?}",)
            }

            Self::UnsupportedManifestSchema { schema } => write!(
                formatter,
                "unsupported diagnostic capsule manifest schema {schema:?}",
            ),

            Self::DestinationExists { path } => write!(
                formatter,
                "diagnostic capsule destination already exists: {}",
                path.display(),
            ),

            Self::EntryLength {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "capsule entry {path:?} length mismatch: expected {expected}, got {actual}",
            ),

            Self::EntryDigest {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "capsule entry {path:?} digest mismatch: expected {expected}, got {actual}",
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

impl Error for DiagnosticCapsuleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonicalization(error) => Some(error),

            Self::Json(error) => Some(error),

            Self::ArtifactVerification(error) => Some(error),

            Self::Io { source, .. } => Some(source),

            Self::ReportMismatch { .. }
            | Self::RemediationReportMismatch { .. }
            | Self::RemediationPlanMismatch { .. }
            | Self::PolicyDenied { .. }
            | Self::DuplicateEntry { .. }
            | Self::DuplicateManifestEntry { .. }
            | Self::InvalidEntryPath { .. }
            | Self::UnsupportedManifestSchema { .. }
            | Self::DestinationExists { .. }
            | Self::EntryLength { .. }
            | Self::EntryDigest { .. } => None,
        }
    }
}

fn rendered_filename(format: RenderedFormat) -> &'static str {
    match format {
        RenderedFormat::Html => "report.html",

        RenderedFormat::Markdown => "report.md",

        RenderedFormat::PlainText => "report.txt",

        RenderedFormat::CompilerText => "compiler.txt",

        RenderedFormat::AuditTranscript => "report.audit",
    }
}

fn validate_destination(destination: &Path) -> Result<(), DiagnosticCapsuleError> {
    if destination.as_os_str().is_empty() || destination.file_name().is_none() {
        return Err(DiagnosticCapsuleError::InvalidEntryPath {
            path: destination.display().to_string(),
        });
    }

    Ok(())
}

fn validate_entry_path(path: &str) -> Result<(), DiagnosticCapsuleError> {
    if path.is_empty() {
        return Err(DiagnosticCapsuleError::InvalidEntryPath {
            path: path.to_owned(),
        });
    }

    let path_value = Path::new(path);

    let mut saw_component = false;

    for component in path_value.components() {
        match component {
            Component::Normal(value) if value != OsStr::new("") => {
                saw_component = true;
            }

            _ => {
                return Err(DiagnosticCapsuleError::InvalidEntryPath {
                    path: path.to_owned(),
                });
            }
        }
    }

    if !saw_component {
        return Err(DiagnosticCapsuleError::InvalidEntryPath {
            path: path.to_owned(),
        });
    }

    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), DiagnosticCapsuleError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error("create capsule file", path, source))?;

    file.write_all(bytes)
        .map_err(|source| io_error("write capsule file", path, source))?;

    file.flush()
        .map_err(|source| io_error("flush capsule file", path, source))?;

    file.sync_all()
        .map_err(|source| io_error("synchronize capsule file", path, source))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), DiagnosticCapsuleError> {
    let directory = File::open(path)
        .map_err(|source| io_error("open capsule directory for synchronization", path, source))?;

    directory
        .sync_all()
        .map_err(|source| io_error("synchronize capsule directory", path, source))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), DiagnosticCapsuleError> {
    Ok(())
}

fn sync_directory_tree(root: &Path) -> Result<(), DiagnosticCapsuleError> {
    let mut directories = Vec::new();

    collect_directories(root, &mut directories)?;

    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));

    for directory in directories {
        sync_directory(&directory)?;
    }

    Ok(())
}

fn collect_directories(
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), DiagnosticCapsuleError> {
    output.push(directory.to_path_buf());

    let entries = fs::read_dir(directory).map_err(|source| {
        io_error(
            "read capsule directory during synchronization",
            directory,
            source,
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| {
            io_error(
                "read capsule directory entry during synchronization",
                directory,
                source,
            )
        })?;

        let file_type = entry.file_type().map_err(|source| {
            io_error("read capsule directory entry type", &entry.path(), source)
        })?;

        if file_type.is_dir() {
            collect_directories(&entry.path(), output)?;
        }
    }

    Ok(())
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> DiagnosticCapsuleError {
    DiagnosticCapsuleError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug)]
struct CapsuleStagingDirectory {
    path: PathBuf,
    committed: bool,
}

impl CapsuleStagingDirectory {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn mark_committed(&mut self) {
        self.committed = true;
    }
}

impl Drop for CapsuleStagingDirectory {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
