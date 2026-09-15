use crate::{Edit, TextRange};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackFailure {
    pub file: PathBuf,
    pub error: String,
}

#[derive(Debug)]
pub enum FixError {
    Io(io::Error),

    InvalidRange {
        file: PathBuf,
        range: TextRange,
    },

    InvalidUtf8Boundary {
        file: PathBuf,
        offset: usize,
    },

    StaleEdit {
        file: PathBuf,
        expected: String,
        actual: String,
    },

    StaleInsert {
        file: PathBuf,
        expected_before: String,
    },

    OverlappingEdits {
        file: PathBuf,
    },

    TransactionWrite {
        file: PathBuf,
        source: io::Error,
        rollback_failures: Vec<RollbackFailure>,
    },
}

impl fmt::Display for FixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),

            Self::InvalidRange { file, range } => write!(
                f,
                "invalid edit range {}..{} for {}",
                range.start,
                range.end,
                file.display()
            ),

            Self::InvalidUtf8Boundary { file, offset } => write!(
                f,
                "edit offset {offset} is not a UTF-8 boundary in {}",
                file.display()
            ),

            Self::StaleEdit {
                file,
                expected,
                actual,
            } => write!(
                f,
                "refusing stale edit in {}: expected {:?}, found {:?}",
                file.display(),
                expected,
                actual
            ),

            Self::StaleInsert {
                file,
                expected_before,
            } => write!(
                f,
                "refusing stale insert in {}: text before insertion no longer ends with {:?}",
                file.display(),
                expected_before
            ),

            Self::OverlappingEdits { file } => {
                write!(f, "refusing overlapping edits in {}", file.display())
            }

            Self::TransactionWrite {
                file,
                source,
                rollback_failures,
            } => {
                write!(
                    f,
                    "transactional write failed for {}: {source}",
                    file.display()
                )?;

                if !rollback_failures.is_empty() {
                    write!(
                        f,
                        "; {} rollback operation(s) also failed",
                        rollback_failures.len()
                    )?;
                }

                Ok(())
            }
        }
    }
}

impl Error for FixError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::TransactionWrite { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for FixError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedFile {
    pub(crate) path: PathBuf,
    pub(crate) original: String,
    pub(crate) updated: String,
}

#[derive(Debug)]
pub(crate) struct AppliedTransaction {
    changed: Vec<PreparedFile>,
}

impl AppliedTransaction {
    pub(crate) fn changed_files(&self) -> Vec<PathBuf> {
        self.changed
            .iter()
            .map(|prepared| prepared.path.clone())
            .collect()
    }

    pub(crate) fn rollback(&self) -> Vec<RollbackFailure> {
        rollback_files(&self.changed)
    }
}

pub(crate) fn affected_files(prepared: &[PreparedFile]) -> Vec<PathBuf> {
    prepared
        .iter()
        .filter(|prepared| prepared.original != prepared.updated)
        .map(|prepared| prepared.path.clone())
        .collect()
}

pub(crate) fn prepare_edits(edits: &[&Edit]) -> Result<Vec<PreparedFile>, FixError> {
    let mut by_file: BTreeMap<PathBuf, Vec<&Edit>> = BTreeMap::new();

    for &edit in edits {
        by_file
            .entry(edit.file().to_path_buf())
            .or_default()
            .push(edit);
    }

    let mut prepared = Vec::new();

    for (file, edits) in by_file {
        let original = fs::read_to_string(&file)?;

        validate_edits(&file, &original, &edits)?;

        let updated = apply_edits(&file, original.clone(), &edits)?;

        prepared.push(PreparedFile {
            path: file,
            original,
            updated,
        });
    }

    Ok(prepared)
}

pub(crate) fn apply_transaction(
    prepared: Vec<PreparedFile>,
    backups: bool,
    backup_suffix: &str,
) -> Result<AppliedTransaction, FixError> {
    let changed: Vec<PreparedFile> = prepared
        .into_iter()
        .filter(|prepared| prepared.original != prepared.updated)
        .collect();

    /*
     * Backups are created before the first target file is modified.
     *
     * A backup failure can therefore abort the transaction without
     * leaving the source tree partially modified.
     */
    if backups {
        for prepared in &changed {
            let backup = backup_path(&prepared.path, backup_suffix);

            atomic_write(&backup, &prepared.original)?;
        }
    }
    for (applied_count, prepared) in changed.iter().enumerate() {
        if let Err(source) = atomic_write(&prepared.path, &prepared.updated) {
            /*
             * `applied_count` is the current index, so every file before
             * this position was written successfully and must be restored.
             */
            let rollback_failures = rollback_files(&changed[..applied_count]);

            return Err(FixError::TransactionWrite {
                file: prepared.path.clone(),
                source,
                rollback_failures,
            });
        }
    }

    Ok(AppliedTransaction { changed })
}

fn rollback_files(prepared: &[PreparedFile]) -> Vec<RollbackFailure> {
    let mut failures = Vec::new();

    /*
     * Restore in reverse write order.
     */
    for prepared in prepared.iter().rev() {
        if let Err(error) = atomic_write(&prepared.path, &prepared.original) {
            failures.push(RollbackFailure {
                file: prepared.path.clone(),
                error: error.to_string(),
            });
        }
    }

    failures
}

fn validate_edits(file: &Path, content: &str, edits: &[&Edit]) -> Result<(), FixError> {
    let mut spans = Vec::new();

    for edit in edits {
        match edit {
            Edit::Replace {
                range, expected, ..
            }
            | Edit::Delete {
                range, expected, ..
            } => {
                validate_range(file, content, *range)?;

                let actual = &content[range.start..range.end];

                if actual != expected {
                    return Err(FixError::StaleEdit {
                        file: file.to_path_buf(),
                        expected: expected.clone(),
                        actual: actual.to_owned(),
                    });
                }

                spans.push((range.start, range.end));
            }

            Edit::Insert {
                offset,
                expected_before,
                ..
            } => {
                if *offset > content.len() {
                    return Err(FixError::InvalidRange {
                        file: file.to_path_buf(),
                        range: TextRange::new(*offset, *offset),
                    });
                }

                if !content.is_char_boundary(*offset) {
                    return Err(FixError::InvalidUtf8Boundary {
                        file: file.to_path_buf(),
                        offset: *offset,
                    });
                }

                if let Some(expected_before) = expected_before
                    && !content[..*offset].ends_with(expected_before)
                {
                    return Err(FixError::StaleInsert {
                        file: file.to_path_buf(),
                        expected_before: expected_before.clone(),
                    });
                }

                spans.push((*offset, *offset));
            }
        }
    }

    spans.sort_unstable();

    for pair in spans.windows(2) {
        let (first_start, first_end) = pair[0];

        let (second_start, second_end) = pair[1];

        let overlaps = first_end > second_start;

        let duplicate_insert =
            first_start == first_end && second_start == second_end && first_start == second_start;

        if overlaps || duplicate_insert {
            return Err(FixError::OverlappingEdits {
                file: file.to_path_buf(),
            });
        }
    }

    Ok(())
}

fn validate_range(file: &Path, content: &str, range: TextRange) -> Result<(), FixError> {
    if range.start > range.end || range.end > content.len() {
        return Err(FixError::InvalidRange {
            file: file.to_path_buf(),
            range,
        });
    }

    for offset in [range.start, range.end] {
        if !content.is_char_boundary(offset) {
            return Err(FixError::InvalidUtf8Boundary {
                file: file.to_path_buf(),
                offset,
            });
        }
    }

    Ok(())
}

fn apply_edits(file: &Path, mut content: String, edits: &[&Edit]) -> Result<String, FixError> {
    let mut edits = edits.to_vec();

    /*
     * Applying edits from highest offset to lowest prevents an earlier
     * edit from shifting offsets required by later edits.
     */
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start()));

    for edit in edits {
        match edit {
            Edit::Replace {
                range, replacement, ..
            } => {
                validate_range(file, &content, *range)?;

                content.replace_range(range.start..range.end, replacement);
            }

            Edit::Delete { range, .. } => {
                validate_range(file, &content, *range)?;

                content.replace_range(range.start..range.end, "");
            }

            Edit::Insert { offset, text, .. } => {
                if *offset > content.len() {
                    return Err(FixError::InvalidRange {
                        file: file.to_path_buf(),
                        range: TextRange::new(*offset, *offset),
                    });
                }

                if !content.is_char_boundary(*offset) {
                    return Err(FixError::InvalidUtf8Boundary {
                        file: file.to_path_buf(),
                        offset: *offset,
                    });
                }

                content.insert_str(*offset, text);
            }
        }
    }

    Ok(content)
}

fn backup_path(file: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", file.display(), suffix))
}

fn atomic_write(file: &Path, contents: &str) -> io::Result<()> {
    let parent = file.parent().unwrap_or_else(|| Path::new("."));

    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)?;
    }

    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("diagprint");

    let temporary = parent.join(format!(".{name}.diagprint-{}.tmp", Uuid::now_v7()));

    fs::write(&temporary, contents)?;

    if let Ok(metadata) = fs::metadata(file) {
        fs::set_permissions(&temporary, metadata.permissions())?;
    }

    if let Err(error) = fs::rename(&temporary, file) {
        let _ = fs::remove_file(&temporary);

        return Err(error);
    }

    Ok(())
}
