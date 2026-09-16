use crate::{
    ArtifactDigest, ArtifactWriteError, ArtifactWriter, ExportedArtifact,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    ffi::OsStr,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

/// Stable schema identifier for immutable generation manifests.
pub const MANIFEST_V1_SCHEMA: &str = "diagprint.manifest/v1";

const GENERATION_WIDTH: usize = 20;
const LOCK_FILE: &str = ".diagprint-store.lock";

/// Link from one committed manifest to its predecessor.
///
/// The link uses the exact-byte digest of the previous manifest, producing an
/// append-only hash chain over committed artifact generations.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct PreviousGeneration {
    pub generation: u64,
    pub manifest_digest: ArtifactDigest,
}

/// Immutable commit record for one artifact generation.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub struct GenerationManifest {
    pub schema: String,
    pub generation: u64,

    pub artifact_name: String,
    pub artifact_digest: ArtifactDigest,
    pub artifact_byte_length: usize,

    pub receipt_name: String,
    pub receipt_digest: ArtifactDigest,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<PreviousGeneration>,
}

/// One committed generation resolved from the artifact store.
#[derive(Debug, Clone)]
pub struct CommittedGeneration {
    manifest: GenerationManifest,
    manifest_digest: ArtifactDigest,
    manifest_path: PathBuf,
    generation_directory: PathBuf,
    artifact_path: PathBuf,
    receipt_path: PathBuf,
}

impl CommittedGeneration {
    pub fn manifest(&self) -> &GenerationManifest {
        &self.manifest
    }

    pub const fn manifest_digest(&self) -> ArtifactDigest {
        self.manifest_digest
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn generation_directory(&self) -> &Path {
        &self.generation_directory
    }

    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }

    pub fn receipt_path(&self) -> &Path {
        &self.receipt_path
    }
}

/// Append-only store of immutable artifact generations.
///
/// The store layout is:
///
/// ```text
/// root/
/// ├── generations/
/// │   ├── 00000000000000000001/
/// │   └── 00000000000000000002/
/// └── manifests/
///     ├── 00000000000000000001.json
///     └── 00000000000000000002.json
/// ```
///
/// A generation directory is not considered committed until its corresponding
/// manifest exists.
///
/// The current generation is the highest committed manifest number. No mutable
/// current-pointer file is required.
#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

/// Failure while committing, loading, or verifying artifact generations.
#[derive(Debug)]
pub enum ArtifactStoreError {
    Busy {
        path: PathBuf,
    },

    InvalidArtifactName {
        name: String,
    },

    GenerationOverflow,

    ArtifactWrite(ArtifactWriteError),

    ManifestSerialization(serde_json::Error),

    ManifestParse {
        path: PathBuf,
        source: serde_json::Error,
    },

    InvalidManifest {
        path: PathBuf,
        reason: &'static str,
    },

    ChainMismatch {
        generation: u64,
    },

    LengthMismatch {
        path: PathBuf,
        expected: usize,
        actual: usize,
    },

    DigestMismatch {
        path: PathBuf,
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },

    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ArtifactStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy { path } => write!(
                formatter,
                "artifact store is busy: {}",
                path.display(),
            ),

            Self::InvalidArtifactName { name } => write!(
                formatter,
                "artifact filename must be one normal path component: {name:?}",
            ),

            Self::GenerationOverflow => {
                formatter.write_str(
                    "artifact generation counter overflowed",
                )
            }

            Self::ArtifactWrite(error) => {
                write!(formatter, "artifact write failed: {error}")
            }

            Self::ManifestSerialization(error) => write!(
                formatter,
                "failed to serialize generation manifest: {error}",
            ),

            Self::ManifestParse { path, source } => write!(
                formatter,
                "failed to parse generation manifest {}: {source}",
                path.display(),
            ),

            Self::InvalidManifest { path, reason } => write!(
                formatter,
                "invalid generation manifest {}: {reason}",
                path.display(),
            ),

            Self::ChainMismatch { generation } => write!(
                formatter,
                "generation manifest chain mismatch at generation {generation}",
            ),

            Self::LengthMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact length mismatch for {}: expected {expected}, got {actual}",
                path.display(),
            ),

            Self::DigestMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "artifact digest mismatch for {}: expected {expected}, got {actual}",
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

impl Error for ArtifactStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ArtifactWrite(error) => Some(error),

            Self::ManifestSerialization(error) => Some(error),

            Self::ManifestParse { source, .. } => Some(source),

            Self::Io { source, .. } => Some(source),

            Self::Busy { .. }
            | Self::InvalidArtifactName { .. }
            | Self::GenerationOverflow
            | Self::InvalidManifest { .. }
            | Self::ChainMismatch { .. }
            | Self::LengthMismatch { .. }
            | Self::DigestMismatch { .. } => None,
        }
    }
}

impl From<ArtifactWriteError> for ArtifactStoreError {
    fn from(error: ArtifactWriteError) -> Self {
        Self::ArtifactWrite(error)
    }
}

impl ArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Commits a new immutable artifact generation.
    ///
    /// Writers are serialized with an OS-level exclusive file lock. The lock
    /// is automatically released by the operating system if the process exits.
    pub fn commit(
        &self,
        exported: &ExportedArtifact,
        artifact_name: &str,
    ) -> Result<CommittedGeneration, ArtifactStoreError> {
        validate_artifact_name(artifact_name)?;

        self.ensure_layout()?;

        let _lock = self.acquire_lock()?;

        let previous = self.current_unlocked()?;

        let generation = self.next_generation_number()?;

        let generation_directory =
            self.generation_directory(generation);

        let persisted = ArtifactWriter::new().write(
            exported,
            &generation_directory,
            artifact_name,
        )?;

        let receipt_bytes =
            fs::read(persisted.receipt_path()).map_err(|source| {
                io_error(
                    "read persisted receipt",
                    persisted.receipt_path(),
                    source,
                )
            })?;

        let receipt_digest =
            ArtifactDigest::compute(&receipt_bytes);

        let receipt_name =
            format!("{artifact_name}.receipt.json");

        let manifest = GenerationManifest {
            schema: MANIFEST_V1_SCHEMA.to_owned(),
            generation,

            artifact_name: artifact_name.to_owned(),
            artifact_digest:
                persisted.artifact_digest(),
            artifact_byte_length:
                persisted.byte_length(),

            receipt_name,
            receipt_digest,

            previous: previous
                .as_ref()
                .map(|previous| PreviousGeneration {
                    generation:
                        previous.manifest.generation,

                    manifest_digest:
                        previous.manifest_digest,
                }),
        };

        let manifest_bytes =
            serde_json::to_vec_pretty(&manifest)
                .map_err(
                    ArtifactStoreError::ManifestSerialization,
                )?;

        let manifest_digest =
            ArtifactDigest::compute(&manifest_bytes);

        let manifest_path =
            self.commit_manifest(
                generation,
                &manifest_bytes,
            )?;

        Ok(CommittedGeneration {
            artifact_path:
                persisted.artifact_path().to_path_buf(),

            receipt_path:
                persisted.receipt_path().to_path_buf(),

            generation_directory,

            manifest_path,
            manifest_digest,
            manifest,
        })
    }

    /// Returns the highest committed generation.
    ///
    /// Uncommitted/orphan generation directories are ignored because the
    /// manifest is the generation commit record.
    pub fn current(
        &self,
    ) -> Result<Option<CommittedGeneration>, ArtifactStoreError> {
        self.ensure_layout()?;
        self.current_unlocked()
    }

    /// Verifies the complete committed manifest chain and every referenced
    /// artifact/receipt pair.
    ///
    /// Returns the number of committed generations verified.
    pub fn verify_history(
        &self,
    ) -> Result<usize, ArtifactStoreError> {
        self.ensure_layout()?;

        let generations = self.manifest_numbers()?;

        let mut previous:
            Option<(u64, ArtifactDigest)> = None;

        for generation in &generations {
            let committed =
                self.load_generation(*generation)?;

            match (
                previous,
                committed.manifest.previous,
            ) {
                (None, None) => {}

                (
                    Some((
                        previous_generation,
                        previous_digest,
                    )),
                    Some(link),
                ) if link.generation
                    == previous_generation
                    && link.manifest_digest
                        == previous_digest => {}

                _ => {
                    return Err(
                        ArtifactStoreError::ChainMismatch {
                            generation: *generation,
                        },
                    );
                }
            }

            self.verify_generation_files(&committed)?;

            previous = Some((
                committed.manifest.generation,
                committed.manifest_digest,
            ));
        }

        Ok(generations.len())
    }

    fn ensure_layout(
        &self,
    ) -> Result<(), ArtifactStoreError> {
        fs::create_dir_all(&self.root).map_err(
            |source| {
                io_error(
                    "create artifact store",
                    &self.root,
                    source,
                )
            },
        )?;

        let generations =
            self.generations_directory();

        fs::create_dir_all(&generations).map_err(
            |source| {
                io_error(
                    "create generations directory",
                    &generations,
                    source,
                )
            },
        )?;

        let manifests =
            self.manifests_directory();

        fs::create_dir_all(&manifests).map_err(
            |source| {
                io_error(
                    "create manifests directory",
                    &manifests,
                    source,
                )
            },
        )
    }

    fn acquire_lock(
        &self,
    ) -> Result<StoreLock, ArtifactStoreError> {
        let path = self.root.join(LOCK_FILE);

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| {
                io_error(
                    "open artifact store lock",
                    &path,
                    source,
                )
            })?;

        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(StoreLock { file }),

            Err(source)
                if source.kind()
                    == io::ErrorKind::WouldBlock =>
            {
                Err(ArtifactStoreError::Busy { path })
            }

            Err(source) => Err(io_error(
                "lock artifact store",
                &path,
                source,
            )),
        }
    }

    fn current_unlocked(
        &self,
    ) -> Result<Option<CommittedGeneration>, ArtifactStoreError> {
        let generations = self.manifest_numbers()?;

        generations
            .last()
            .copied()
            .map(|generation| {
                self.load_generation(generation)
            })
            .transpose()
    }

    fn next_generation_number(
        &self,
    ) -> Result<u64, ArtifactStoreError> {
        let manifest_max =
            self.manifest_numbers()?
                .into_iter()
                .max();

        let directory_max =
            self.generation_numbers()?
                .into_iter()
                .max();

        manifest_max
            .into_iter()
            .chain(directory_max)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(
                ArtifactStoreError::GenerationOverflow,
            )
    }

    fn load_generation(
        &self,
        generation: u64,
    ) -> Result<CommittedGeneration, ArtifactStoreError> {
        let manifest_path =
            self.manifest_path(generation);

        let bytes =
            fs::read(&manifest_path).map_err(
                |source| {
                    io_error(
                        "read generation manifest",
                        &manifest_path,
                        source,
                    )
                },
            )?;

        let manifest:
            GenerationManifest =
            serde_json::from_slice(&bytes)
                .map_err(|source| {
                    ArtifactStoreError::ManifestParse {
                        path:
                            manifest_path.clone(),
                        source,
                    }
                })?;

        if manifest.schema != MANIFEST_V1_SCHEMA {
            return Err(
                ArtifactStoreError::InvalidManifest {
                    path: manifest_path,
                    reason:
                        "unsupported manifest schema",
                },
            );
        }

        if manifest.generation != generation {
            return Err(
                ArtifactStoreError::InvalidManifest {
                    path: manifest_path,
                    reason:
                        "manifest generation does not match filename",
                },
            );
        }

        if !is_valid_artifact_name(
            &manifest.artifact_name,
        ) {
            return Err(
                ArtifactStoreError::InvalidManifest {
                    path: manifest_path,
                    reason:
                        "invalid artifact filename",
                },
            );
        }

        let expected_receipt = format!(
            "{}.receipt.json",
            manifest.artifact_name,
        );

        if manifest.receipt_name
            != expected_receipt
            || !is_valid_artifact_name(
                &manifest.receipt_name,
            )
        {
            return Err(
                ArtifactStoreError::InvalidManifest {
                    path: manifest_path,
                    reason:
                        "invalid receipt filename",
                },
            );
        }

        let manifest_digest =
            ArtifactDigest::compute(&bytes);

        let generation_directory =
            self.generation_directory(generation);

        let artifact_path =
            generation_directory.join(
                &manifest.artifact_name,
            );

        let receipt_path =
            generation_directory.join(
                &manifest.receipt_name,
            );

        Ok(CommittedGeneration {
            manifest,
            manifest_digest,
            manifest_path,
            generation_directory,
            artifact_path,
            receipt_path,
        })
    }

    fn verify_generation_files(
        &self,
        generation: &CommittedGeneration,
    ) -> Result<(), ArtifactStoreError> {
        let artifact =
            fs::read(&generation.artifact_path)
                .map_err(|source| {
                    io_error(
                        "read generation artifact",
                        &generation.artifact_path,
                        source,
                    )
                })?;

        if artifact.len()
            != generation
                .manifest
                .artifact_byte_length
        {
            return Err(
                ArtifactStoreError::LengthMismatch {
                    path:
                        generation.artifact_path.clone(),

                    expected:
                        generation
                            .manifest
                            .artifact_byte_length,

                    actual: artifact.len(),
                },
            );
        }

        let artifact_digest =
            ArtifactDigest::compute(&artifact);

        if artifact_digest
            != generation.manifest.artifact_digest
        {
            return Err(
                ArtifactStoreError::DigestMismatch {
                    path:
                        generation.artifact_path.clone(),

                    expected:
                        generation
                            .manifest
                            .artifact_digest,

                    actual: artifact_digest,
                },
            );
        }

        let receipt =
            fs::read(&generation.receipt_path)
                .map_err(|source| {
                    io_error(
                        "read generation receipt",
                        &generation.receipt_path,
                        source,
                    )
                })?;

        let receipt_digest =
            ArtifactDigest::compute(&receipt);

        if receipt_digest
            != generation.manifest.receipt_digest
        {
            return Err(
                ArtifactStoreError::DigestMismatch {
                    path:
                        generation.receipt_path.clone(),

                    expected:
                        generation
                            .manifest
                            .receipt_digest,

                    actual: receipt_digest,
                },
            );
        }

        Ok(())
    }

    fn commit_manifest(
        &self,
        generation: u64,
        bytes: &[u8],
    ) -> Result<PathBuf, ArtifactStoreError> {
        let manifests =
            self.manifests_directory();

        let final_path =
            self.manifest_path(generation);

        let staging_path = manifests.join(format!(
            ".diagprint-manifest-{}.json",
            Uuid::now_v7(),
        ));

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging_path)
            .map_err(|source| {
                io_error(
                    "create staged manifest",
                    &staging_path,
                    source,
                )
            })?;

        file.write_all(bytes).map_err(
            |source| {
                io_error(
                    "write staged manifest",
                    &staging_path,
                    source,
                )
            },
        )?;

        file.flush().map_err(|source| {
            io_error(
                "flush staged manifest",
                &staging_path,
                source,
            )
        })?;

        file.sync_all().map_err(|source| {
            io_error(
                "synchronize staged manifest",
                &staging_path,
                source,
            )
        })?;

        drop(file);

        let link_result =
            fs::hard_link(
                &staging_path,
                &final_path,
            );

        if let Err(source) = link_result {
            let _ =
                fs::remove_file(&staging_path);

            return Err(io_error(
                "commit generation manifest",
                &final_path,
                source,
            ));
        }

        // The committed manifest now has its own directory entry. Removing the
        // staging link cannot remove the committed manifest.
        let _ = fs::remove_file(&staging_path);

        sync_directory(&manifests)?;

        Ok(final_path)
    }

    fn manifest_numbers(
        &self,
    ) -> Result<Vec<u64>, ArtifactStoreError> {
        let directory =
            self.manifests_directory();

        let mut values = Vec::new();

        for entry in fs::read_dir(&directory)
            .map_err(|source| {
                io_error(
                    "read manifests directory",
                    &directory,
                    source,
                )
            })?
        {
            let entry = entry.map_err(|source| {
                io_error(
                    "read manifest directory entry",
                    &directory,
                    source,
                )
            })?;

            if !entry
                .file_type()
                .map_err(|source| {
                    io_error(
                        "read manifest entry type",
                        &entry.path(),
                        source,
                    )
                })?
                .is_file()
            {
                continue;
            }

            if let Some(generation) =
                parse_manifest_name(
                    &entry.file_name(),
                )
            {
                values.push(generation);
            }
        }

        values.sort_unstable();
        values.dedup();

        Ok(values)
    }

    fn generation_numbers(
        &self,
    ) -> Result<Vec<u64>, ArtifactStoreError> {
        let directory =
            self.generations_directory();

        let mut values = Vec::new();

        for entry in fs::read_dir(&directory)
            .map_err(|source| {
                io_error(
                    "read generations directory",
                    &directory,
                    source,
                )
            })?
        {
            let entry = entry.map_err(|source| {
                io_error(
                    "read generation directory entry",
                    &directory,
                    source,
                )
            })?;

            if !entry
                .file_type()
                .map_err(|source| {
                    io_error(
                        "read generation entry type",
                        &entry.path(),
                        source,
                    )
                })?
                .is_dir()
            {
                continue;
            }

            if let Some(generation) =
                parse_generation_name(
                    &entry.file_name(),
                )
            {
                values.push(generation);
            }
        }

        values.sort_unstable();
        values.dedup();

        Ok(values)
    }

    fn generations_directory(&self) -> PathBuf {
        self.root.join("generations")
    }

    fn manifests_directory(&self) -> PathBuf {
        self.root.join("manifests")
    }

    fn generation_directory(
        &self,
        generation: u64,
    ) -> PathBuf {
        self.generations_directory()
            .join(generation_name(generation))
    }

    fn manifest_path(
        &self,
        generation: u64,
    ) -> PathBuf {
        self.manifests_directory().join(
            format!(
                "{}.json",
                generation_name(generation),
            ),
        )
    }
}

struct StoreLock {
    file: File,
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ =
            fs2::FileExt::unlock(&self.file);
    }
}

fn validate_artifact_name(
    name: &str,
) -> Result<(), ArtifactStoreError> {
    if is_valid_artifact_name(name) {
        Ok(())
    } else {
        Err(
            ArtifactStoreError::InvalidArtifactName {
                name: name.to_owned(),
            },
        )
    }
}

fn is_valid_artifact_name(name: &str) -> bool {
    let path = Path::new(name);
    let mut components = path.components();

    matches!(
        components.next(),
        Some(Component::Normal(component))
            if component == OsStr::new(name)
    ) && components.next().is_none()
}

fn generation_name(
    generation: u64,
) -> String {
    format!(
        "{generation:0width$}",
        width = GENERATION_WIDTH,
    )
}

fn parse_generation_name(
    name: &OsStr,
) -> Option<u64> {
    let name = name.to_str()?;

    if name.len() != GENERATION_WIDTH
        || !name.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }

    name.parse().ok()
}

fn parse_manifest_name(
    name: &OsStr,
) -> Option<u64> {
    let name = name.to_str()?;

    let generation =
        name.strip_suffix(".json")?;

    if generation.len() != GENERATION_WIDTH
        || !generation
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }

    generation.parse().ok()
}

#[cfg(unix)]
fn sync_directory(
    path: &Path,
) -> Result<(), ArtifactStoreError> {
    let directory =
        File::open(path).map_err(|source| {
            io_error(
                "open directory for synchronization",
                path,
                source,
            )
        })?;

    directory.sync_all().map_err(|source| {
        io_error(
            "synchronize directory",
            path,
            source,
        )
    })
}

#[cfg(not(unix))]
fn sync_directory(
    _path: &Path,
) -> Result<(), ArtifactStoreError> {
    Ok(())
}

fn io_error(
    operation: &'static str,
    path: &Path,
    source: io::Error,
) -> ArtifactStoreError {
    ArtifactStoreError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
