use diagprint::SourceRevision;
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspError {
    MissingLabel,

    MissingDocument {
        source: String,
    },

    MissingSource {
        source: String,
    },

    MissingDocumentVersion {
        source: String,
    },

    RevisionMismatch {
        source: String,
        expected: SourceRevision,
        actual: Option<SourceRevision>,
    },

    InvalidLine {
        source: String,
        line: u32,
    },

    InvalidColumn {
        source: String,
        line: u32,
        column: u32,
    },

    InvalidByteRange {
        source: String,
        start: usize,
        end: usize,
    },

    InvalidUtf8Boundary {
        source: String,
        offset: usize,
    },

    GuardMismatch {
        source: String,
        detail: String,
    },

    UnsafeEdit {
        source: String,
        reason: String,
    },

    OverlappingEdits {
        source: String,
    },

    NonUtf8Path,
    PositionOverflow,

    UnsupportedPositionEncoding {
        encoding: String,
    },
}

impl fmt::Display for LspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLabel => f.write_str("diagnostic has no source label"),

            Self::MissingDocument { source } => {
                write!(f, "no LSP document is registered for {source}")
            }

            Self::MissingSource { source } => {
                write!(f, "source text is unavailable for {source}")
            }

            Self::MissingDocumentVersion { source } => {
                write!(f, "no LSP document version is registered for {source}")
            }

            Self::RevisionMismatch {
                source,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "source revision mismatch for {source}: expected r{expected}, got "
                )?;

                match actual {
                    Some(actual) => write!(f, "r{actual}"),
                    None => f.write_str("no revision"),
                }
            }

            Self::InvalidLine { source, line } => {
                write!(f, "invalid line {line} for {source}")
            }

            Self::InvalidColumn {
                source,
                line,
                column,
            } => {
                write!(f, "invalid column {column} on line {line} for {source}")
            }

            Self::InvalidByteRange { source, start, end } => {
                write!(f, "invalid byte range {start}..{end} for {source}")
            }

            Self::InvalidUtf8Boundary { source, offset } => {
                write!(
                    f,
                    "byte offset {offset} is not a UTF-8 boundary in {source}"
                )
            }

            Self::GuardMismatch { source, detail } => {
                write!(f, "edit guard failed for {source}: {detail}")
            }

            Self::UnsafeEdit { source, reason } => {
                write!(f, "unsafe edit for {source}: {reason}")
            }

            Self::OverlappingEdits { source } => {
                write!(f, "overlapping edits for {source}")
            }

            Self::NonUtf8Path => f.write_str("edit path is not valid UTF-8"),

            Self::PositionOverflow => f.write_str("LSP position exceeds u32 range"),

            Self::UnsupportedPositionEncoding { encoding } => {
                write!(f, "unsupported LSP position encoding: {encoding}")
            }
        }
    }
}

impl Error for LspError {}
