use crate::LspError;
use diagprint::{Label, SourceLocation, SourceSnapshot};
use lsp_types::{Position, PositionEncodingKind, Range};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PositionEncoding {
    Utf8,

    #[default]
    Utf16,

    Utf32,
}

impl PositionEncoding {
    pub fn from_lsp(encoding: &PositionEncodingKind) -> Result<Self, LspError> {
        match encoding.as_str() {
            "utf-8" => Ok(Self::Utf8),
            "utf-16" => Ok(Self::Utf16),
            "utf-32" => Ok(Self::Utf32),

            other => Err(LspError::UnsupportedPositionEncoding {
                encoding: other.to_owned(),
            }),
        }
    }

    pub fn as_lsp(self) -> PositionEncodingKind {
        match self {
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf16 => PositionEncodingKind::UTF16,
            Self::Utf32 => PositionEncodingKind::UTF32,
        }
    }
}

pub(crate) fn verify_revision(
    location: &SourceLocation,
    sources: &SourceSnapshot,
) -> Result<(), LspError> {
    let entry = sources
        .entry(&location.file)
        .ok_or_else(|| LspError::MissingSource {
            source: location.file.clone(),
        })?;

    if let Some(expected) = location.revision {
        let actual = entry.revision();

        if actual != expected {
            return Err(LspError::RevisionMismatch {
                source: location.file.clone(),
                expected,
                actual: Some(actual),
            });
        }
    }

    Ok(())
}

pub(crate) fn label_range(
    label: &Label,
    sources: &SourceSnapshot,
    encoding: PositionEncoding,
) -> Result<Range, LspError> {
    verify_revision(&label.location, sources)?;

    let source = sources
        .get(&label.location.file)
        .ok_or_else(|| LspError::MissingSource {
            source: label.location.file.clone(),
        })?;

    let line_number = label.location.line;

    let line_index = line_number
        .checked_sub(1)
        .ok_or_else(|| LspError::InvalidLine {
            source: label.location.file.clone(),
            line: line_number,
        })?;

    let raw_line =
        source
            .split('\n')
            .nth(line_index as usize)
            .ok_or_else(|| LspError::InvalidLine {
                source: label.location.file.clone(),
                line: line_number,
            })?;

    let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);

    let column = label.location.column.unwrap_or(1);

    let scalar_start = column
        .checked_sub(1)
        .ok_or_else(|| LspError::InvalidColumn {
            source: label.location.file.clone(),
            line: line_number,
            column,
        })? as usize;

    let scalar_count = line.chars().count();

    if scalar_start > scalar_count {
        return Err(LspError::InvalidColumn {
            source: label.location.file.clone(),
            line: line_number,
            column,
        });
    }

    let length = label.length.unwrap_or(0);

    let scalar_end = scalar_start
        .checked_add(length)
        .ok_or(LspError::PositionOverflow)?;

    if scalar_end > scalar_count {
        return Err(LspError::InvalidColumn {
            source: label.location.file.clone(),
            line: line_number,
            column,
        });
    }

    let start_byte = byte_for_scalar(line, scalar_start);

    let end_byte = byte_for_scalar(line, scalar_end);

    let start_character = encoded_units(&line[..start_byte], encoding)?;

    let end_character = encoded_units(&line[..end_byte], encoding)?;

    Ok(Range::new(
        Position::new(line_index, start_character),
        Position::new(line_index, end_character),
    ))
}

pub(crate) fn offset_position(
    source_name: &str,
    source: &str,
    offset: usize,
    encoding: PositionEncoding,
) -> Result<Position, LspError> {
    if offset > source.len() {
        return Err(LspError::InvalidByteRange {
            source: source_name.to_owned(),
            start: offset,
            end: offset,
        });
    }

    if !source.is_char_boundary(offset) {
        return Err(LspError::InvalidUtf8Boundary {
            source: source_name.to_owned(),
            offset,
        });
    }

    let prefix = &source[..offset];

    let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count())
        .map_err(|_| LspError::PositionOverflow)?;

    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);

    let line_prefix = &source[line_start..offset];

    let character = encoded_units(line_prefix, encoding)?;

    Ok(Position::new(line, character))
}

fn byte_for_scalar(text: &str, scalar_index: usize) -> usize {
    if scalar_index == text.chars().count() {
        return text.len();
    }

    text.char_indices()
        .nth(scalar_index)
        .map_or(text.len(), |(index, _)| index)
}

fn encoded_units(text: &str, encoding: PositionEncoding) -> Result<u32, LspError> {
    let units = match encoding {
        PositionEncoding::Utf8 => text.len(),

        PositionEncoding::Utf16 => text.encode_utf16().count(),

        PositionEncoding::Utf32 => text.chars().count(),
    };

    u32::try_from(units).map_err(|_| LspError::PositionOverflow)
}
