use crate::{
    ArtifactDigest, ArtifactVerificationError, ExportedArtifact, render::RenderedArtifact,
};
use serde::Serialize;
use std::{
    error::Error,
    ffi::OsStr,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

/// Persists verified artifacts as create-only transactional directories.
///
/// The writer stages the artifact and receipt together in a sibling directory,
/// verifies the staged artifact, synchronizes the files, and then commits the
/// complete directory with one filesystem rename.
///
/// Existing destinations are never intentionally overwritten.
///
/// A successful destination contains:
///
/// ~~~text
/// <artifact filename>
/// <artifact filename>.receipt.json
/// ~~~
///
/// This deliberately separates durable artifacts from append-oriented
/// diagnostic sinks and [`crate::Reporter`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ArtifactWriter;

/// Result of one committed artifact transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedArtifact {
    directory: PathBuf,
    artifact_path: PathBuf,
    receipt_path: PathBuf,
    artifact_digest: ArtifactDigest,
    byte_length: usize,
}

impl PersistedArtifact {
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }

    pub fn receipt_path(&self) -> &Path {
        &self.receipt_path
    }

    pub const fn artifact_digest(&self) -> ArtifactDigest {
        self.artifact_digest
    }

    pub const fn byte_length(&self) -> usize {
        self.byte_length
    }
}

/// Exact byte identity recorded by an artifact receipt.
#[derive(Debug, Clone, Copy)]
struct ArtifactIdentity {
    digest: ArtifactDigest,
    byte_length: usize,
}

impl ArtifactIdentity {
    const fn new(digest: ArtifactDigest, byte_length: usize) -> Self {
        Self {
            digest,
            byte_length,
        }
    }
}

/// Failure while transactionally persisting an exported artifact.
#[derive(Debug)]
pub enum ArtifactWriteError {
    InvalidDestination {
        path: PathBuf,
    },

    InvalidArtifactName {
        name: String,
    },

    DestinationExists {
        path: PathBuf,
    },

    Verification(ArtifactVerificationError),

    ReceiptSerialization(serde_json::Error),

    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ArtifactWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDestination { path } => write!(
                formatter,
                "invalid artifact destination: {}",
                path.display(),
            ),

            Self::InvalidArtifactName { name } => write!(
                formatter,
                "artifact filename must be one normal path component: {name:?}",
            ),

            Self::DestinationExists { path } => write!(
                formatter,
                "artifact destination already exists: {}",
                path.display(),
            ),

            Self::Verification(error) => write!(
                formatter,
                "artifact verification failed before persistence: {error}",
            ),

            Self::ReceiptSerialization(error) => {
                write!(formatter, "failed to serialize artifact receipt: {error}",)
            }

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

impl Error for ArtifactWriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Verification(error) => Some(error),
            Self::ReceiptSerialization(error) => Some(error),
            Self::Io { source, .. } => Some(source),

            Self::InvalidDestination { .. }
            | Self::InvalidArtifactName { .. }
            | Self::DestinationExists { .. } => None,
        }
    }
}

impl From<ArtifactVerificationError> for ArtifactWriteError {
    fn from(error: ArtifactVerificationError) -> Self {
        Self::Verification(error)
    }
}

impl ArtifactWriter {
    pub const fn new() -> Self {
        Self
    }

    /// Persists a delta export and receipt as one create-only transaction.
    pub fn write(
        &self,
        exported: &ExportedArtifact,
        destination: impl AsRef<Path>,
        artifact_name: &str,
    ) -> Result<PersistedArtifact, ArtifactWriteError> {
        let receipt = exported.receipt();

        let identity = ArtifactIdentity::new(receipt.artifact_digest, receipt.byte_length);

        self.write_verified(
            exported.bytes(),
            receipt,
            |bytes| receipt.verify_bytes(bytes),
            identity,
            destination.as_ref(),
            artifact_name,
        )
    }

    /// Persists any verified rendered report artifact as one create-only
    /// transaction.
    ///
    /// HTML, Markdown, and plain-text reports all use this same persistence
    /// path.
    pub fn write_rendered(
        &self,
        artifact: &RenderedArtifact,
        destination: impl AsRef<Path>,
        artifact_name: &str,
    ) -> Result<PersistedArtifact, ArtifactWriteError> {
        let receipt = artifact.receipt();

        let identity = ArtifactIdentity::new(receipt.artifact_digest, receipt.byte_length);

        self.write_verified(
            artifact.bytes(),
            receipt,
            |bytes| receipt.verify_bytes(bytes),
            identity,
            destination.as_ref(),
            artifact_name,
        )
    }

    fn write_verified<R, F>(
        &self,
        bytes: &[u8],
        receipt: &R,
        verify: F,
        identity: ArtifactIdentity,
        destination: &Path,
        artifact_name: &str,
    ) -> Result<PersistedArtifact, ArtifactWriteError>
    where
        R: Serialize,
        F: Fn(&[u8]) -> Result<(), ArtifactVerificationError>,
    {
        validate_destination(destination)?;
        validate_artifact_name(artifact_name)?;

        // Never persist bytes which already disagree with their in-memory
        // receipt.
        verify(bytes)?;

        if destination.exists() {
            return Err(ArtifactWriteError::DestinationExists {
                path: destination.to_path_buf(),
            });
        }

        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        fs::create_dir_all(parent)
            .map_err(|source| io_error("create artifact parent directory", parent, source))?;

        let staging_path = parent.join(format!(".diagprint-stage-{}", Uuid::now_v7(),));

        fs::create_dir(&staging_path).map_err(|source| {
            io_error("create artifact staging directory", &staging_path, source)
        })?;

        let mut staging = StagingDirectory::new(staging_path);

        let staged_artifact = staging.path().join(artifact_name);

        let receipt_name = format!("{artifact_name}.receipt.json");

        let staged_receipt = staging.path().join(&receipt_name);

        write_synced(&staged_artifact, bytes)?;

        // Verify the actual bytes read back from storage before committing the
        // transaction.
        let persisted_bytes = fs::read(&staged_artifact)
            .map_err(|source| io_error("read staged artifact", &staged_artifact, source))?;

        verify(&persisted_bytes)?;

        let receipt_bytes =
            serde_json::to_vec_pretty(receipt).map_err(ArtifactWriteError::ReceiptSerialization)?;

        write_synced(&staged_receipt, &receipt_bytes)?;

        sync_directory(staging.path())?;

        // Recheck immediately before commit. This also handles the ordinary
        // concurrent-writer case without intentionally replacing a completed
        // artifact directory.
        if destination.exists() {
            return Err(ArtifactWriteError::DestinationExists {
                path: destination.to_path_buf(),
            });
        }

        fs::rename(staging.path(), destination).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                ArtifactWriteError::DestinationExists {
                    path: destination.to_path_buf(),
                }
            } else {
                io_error("commit artifact transaction", destination, source)
            }
        })?;

        staging.mark_committed();

        // Persist the directory entry itself where the platform exposes
        // directory fsync semantics.
        sync_directory(parent)?;

        Ok(PersistedArtifact {
            directory: destination.to_path_buf(),

            artifact_path: destination.join(artifact_name),

            receipt_path: destination.join(receipt_name),

            artifact_digest: identity.digest,

            byte_length: identity.byte_length,
        })
    }
}

fn validate_destination(destination: &Path) -> Result<(), ArtifactWriteError> {
    if destination.as_os_str().is_empty() || destination.file_name().is_none() {
        return Err(ArtifactWriteError::InvalidDestination {
            path: destination.to_path_buf(),
        });
    }

    Ok(())
}

fn validate_artifact_name(artifact_name: &str) -> Result<(), ArtifactWriteError> {
    let path = Path::new(artifact_name);

    let mut components = path.components();

    let valid = matches!(
        components.next(),
        Some(Component::Normal(component))
            if component == OsStr::new(artifact_name)
    ) && components.next().is_none();

    if !valid {
        return Err(ArtifactWriteError::InvalidArtifactName {
            name: artifact_name.to_owned(),
        });
    }

    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), ArtifactWriteError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error("create staged artifact file", path, source))?;

    file.write_all(bytes)
        .map_err(|source| io_error("write staged artifact file", path, source))?;

    file.flush()
        .map_err(|source| io_error("flush staged artifact file", path, source))?;

    file.sync_all()
        .map_err(|source| io_error("synchronize staged artifact file", path, source))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), ArtifactWriteError> {
    let directory = File::open(path)
        .map_err(|source| io_error("open directory for synchronization", path, source))?;

    directory
        .sync_all()
        .map_err(|source| io_error("synchronize directory", path, source))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), ArtifactWriteError> {
    Ok(())
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> ArtifactWriteError {
    ArtifactWriteError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug)]
struct StagingDirectory {
    path: PathBuf,
    committed: bool,
}

impl StagingDirectory {
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

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
