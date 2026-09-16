use crate::{
    DocumentMap, LspError, PositionEncoding, diagnostic::to_lsp_diagnostic,
    position::offset_position,
};
use diagprint::{Applicability, Diagnostic, Edit, SourceSnapshot, Suggestion};
use lsp_types::{
    CodeAction, CodeActionDisabled, CodeActionKind, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct ValidatedEdit {
    source: String,
    start: usize,
    end: usize,
    new_text: String,
}

pub(crate) fn code_actions(
    diagnostic: &Diagnostic,
    sources: &SourceSnapshot,
    documents: &DocumentMap,
    encoding: PositionEncoding,
) -> Result<Vec<CodeAction>, LspError> {
    let lsp_diagnostic = to_lsp_diagnostic(diagnostic, sources, documents, encoding)?;

    let mut actions = Vec::with_capacity(diagnostic.suggestions.len());

    for suggestion in &diagnostic.suggestions {
        let mut action = CodeAction {
            title: suggestion.title.clone(),

            kind: Some(CodeActionKind::QUICKFIX),

            diagnostics: Some(vec![lsp_diagnostic.clone()]),

            ..CodeAction::default()
        };

        if !suggestion.commands.is_empty() {
            action.disabled = Some(CodeActionDisabled {
                reason: "diagprint-lsp does not execute or translate suggested commands".to_owned(),
            });

            actions.push(action);
            continue;
        }

        match suggestion.applicability {
            Applicability::Manual => {
                action.disabled = Some(CodeActionDisabled {
                    reason: "manual remediation requires explicit user review".to_owned(),
                });
            }

            Applicability::HasPlaceholders => {
                action.disabled = Some(CodeActionDisabled {
                    reason: "remediation contains placeholders and cannot be applied directly"
                        .to_owned(),
                });
            }

            Applicability::MaybeIncorrect => {
                match workspace_edit(suggestion, sources, documents, encoding, false) {
                    Ok(edit) => {
                        action.edit = Some(edit);

                        action.is_preferred = Some(false);
                    }

                    Err(error) => {
                        action.disabled = Some(CodeActionDisabled {
                            reason: error.to_string(),
                        });
                    }
                }
            }

            Applicability::MachineApplicable => {
                match workspace_edit(suggestion, sources, documents, encoding, true) {
                    Ok(edit) => {
                        action.edit = Some(edit);

                        action.is_preferred = Some(true);
                    }

                    Err(error) => {
                        action.disabled = Some(CodeActionDisabled {
                            reason: error.to_string(),
                        });
                    }
                }
            }
        }

        actions.push(action);
    }

    Ok(actions)
}

fn workspace_edit(
    suggestion: &Suggestion,
    sources: &SourceSnapshot,
    documents: &DocumentMap,
    encoding: PositionEncoding,
    require_machine_guards: bool,
) -> Result<WorkspaceEdit, LspError> {
    if suggestion.edits.is_empty() {
        return Err(LspError::UnsafeEdit {
            source: "<suggestion>".to_owned(),

            reason: "suggestion contains no structured edits".to_owned(),
        });
    }

    let mut grouped: BTreeMap<String, Vec<ValidatedEdit>> = BTreeMap::new();

    for edit in &suggestion.edits {
        let validated = validate_edit(edit, sources, require_machine_guards)?;

        grouped
            .entry(validated.source.clone())
            .or_default()
            .push(validated);
    }

    let mut document_edits = Vec::new();

    for (source_name, mut edits) in grouped {
        validate_non_overlapping(&source_name, &mut edits)?;

        let source = sources
            .get(&source_name)
            .ok_or_else(|| LspError::MissingSource {
                source: source_name.clone(),
            })?;

        let document = documents
            .get(&source_name)
            .ok_or_else(|| LspError::MissingDocument {
                source: source_name.clone(),
            })?;

        let version = document
            .version
            .ok_or_else(|| LspError::MissingDocumentVersion {
                source: source_name.clone(),
            })?;

        let mut text_edits = Vec::new();

        for edit in edits {
            let start = offset_position(&source_name, &source, edit.start, encoding)?;

            let end = offset_position(&source_name, &source, edit.end, encoding)?;

            text_edits.push(OneOf::Left(TextEdit::new(
                Range::new(start, end),
                edit.new_text,
            )));
        }

        document_edits.push(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier::new(
                document.uri.clone(),
                version,
            ),

            edits: text_edits,
        });
    }

    Ok(WorkspaceEdit {
        changes: None,

        document_changes: Some(DocumentChanges::Edits(document_edits)),

        change_annotations: None,
    })
}

fn validate_edit(
    edit: &Edit,
    sources: &SourceSnapshot,
    require_machine_guards: bool,
) -> Result<ValidatedEdit, LspError> {
    match edit {
        Edit::Replace {
            file,
            range,
            expected,
            replacement,
        } => {
            let source_name = source_name(file)?;

            let source = source_text(&source_name, sources)?;

            validate_range(&source_name, &source, range.start, range.end)?;

            if &source[range.start..range.end] != expected {
                return Err(LspError::GuardMismatch {
                    source: source_name,

                    detail: "replacement expected text no longer matches".to_owned(),
                });
            }

            Ok(ValidatedEdit {
                source: source_name,
                start: range.start,
                end: range.end,
                new_text: replacement.clone(),
            })
        }

        Edit::Delete {
            file,
            range,
            expected,
        } => {
            let source_name = source_name(file)?;

            let source = source_text(&source_name, sources)?;

            validate_range(&source_name, &source, range.start, range.end)?;

            if &source[range.start..range.end] != expected {
                return Err(LspError::GuardMismatch {
                    source: source_name,

                    detail: "delete expected text no longer matches".to_owned(),
                });
            }

            Ok(ValidatedEdit {
                source: source_name,
                start: range.start,
                end: range.end,
                new_text: String::new(),
            })
        }

        Edit::Insert {
            file,
            offset,
            expected_before,
            text,
        } => {
            let source_name = source_name(file)?;

            let source = source_text(&source_name, sources)?;

            validate_range(&source_name, &source, *offset, *offset)?;

            if require_machine_guards && expected_before.is_none() {
                return Err(LspError::UnsafeEdit {
                    source: source_name,

                    reason: "machine-applicable insertion has no expected_before guard".to_owned(),
                });
            }

            if let Some(expected_before) = expected_before {
                let prefix = &source[..*offset];

                if !prefix.ends_with(expected_before) {
                    return Err(LspError::GuardMismatch {
                        source: source_name,

                        detail: "insert expected_before guard no longer matches".to_owned(),
                    });
                }
            }

            Ok(ValidatedEdit {
                source: source_name,
                start: *offset,
                end: *offset,
                new_text: text.clone(),
            })
        }
    }
}

fn source_name(path: &std::path::Path) -> Result<String, LspError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or(LspError::NonUtf8Path)
}

fn source_text(
    source_name: &str,
    sources: &SourceSnapshot,
) -> Result<std::sync::Arc<str>, LspError> {
    sources
        .get(source_name)
        .ok_or_else(|| LspError::MissingSource {
            source: source_name.to_owned(),
        })
}

fn validate_range(
    source_name: &str,
    source: &str,
    start: usize,
    end: usize,
) -> Result<(), LspError> {
    if start > end || end > source.len() {
        return Err(LspError::InvalidByteRange {
            source: source_name.to_owned(),
            start,
            end,
        });
    }

    if !source.is_char_boundary(start) {
        return Err(LspError::InvalidUtf8Boundary {
            source: source_name.to_owned(),
            offset: start,
        });
    }

    if !source.is_char_boundary(end) {
        return Err(LspError::InvalidUtf8Boundary {
            source: source_name.to_owned(),
            offset: end,
        });
    }

    Ok(())
}

fn validate_non_overlapping(
    source_name: &str,
    edits: &mut [ValidatedEdit],
) -> Result<(), LspError> {
    edits.sort_by_key(|edit| (edit.start, edit.end));

    for pair in edits.windows(2) {
        let left = &pair[0];
        let right = &pair[1];

        let overlaps = right.start < left.end || right.start == left.start;

        if overlaps {
            return Err(LspError::OverlappingEdits {
                source: source_name.to_owned(),
            });
        }
    }

    Ok(())
}
