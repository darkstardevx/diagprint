use crate::{
    DocumentMap, LspError, PositionEncoding,
    position::{label_range, verify_revision},
};
use diagprint::{Diagnostic as CoreDiagnostic, Label, LabelKind, Severity, SourceSnapshot};
use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location,
    NumberOrString, PublishDiagnosticsParams,
};

pub(crate) fn to_lsp_diagnostic(
    diagnostic: &CoreDiagnostic,
    sources: &SourceSnapshot,
    documents: &DocumentMap,
    encoding: PositionEncoding,
) -> Result<LspDiagnostic, LspError> {
    let main_index = main_label_index(diagnostic)?;

    let main_label = &diagnostic.labels[main_index];

    verify_revision(&main_label.location, sources)?;

    documents
        .get(&main_label.location.file)
        .ok_or_else(|| LspError::MissingDocument {
            source: main_label.location.file.clone(),
        })?;

    let range = label_range(main_label, sources, encoding)?;

    let mut related = Vec::new();

    for (index, label) in diagnostic.labels.iter().enumerate() {
        if index == main_index {
            continue;
        }

        verify_revision(&label.location, sources)?;

        let document =
            documents
                .get(&label.location.file)
                .ok_or_else(|| LspError::MissingDocument {
                    source: label.location.file.clone(),
                })?;

        let range = label_range(label, sources, encoding)?;

        related.push(DiagnosticRelatedInformation {
            location: Location::new(document.uri.clone(), range),

            message: related_message(label),
        });
    }

    Ok(LspDiagnostic {
        range,

        severity: Some(lsp_severity(diagnostic.severity)),

        code: diagnostic
            .code
            .as_ref()
            .map(|code| NumberOrString::String(code.clone())),

        code_description: None,

        source: Some("diagprint".to_owned()),

        message: diagnostic_message(diagnostic),

        related_information: (!related.is_empty()).then_some(related),

        tags: None,

        data: None,
    })
}

pub(crate) fn publish_diagnostic(
    diagnostic: &CoreDiagnostic,
    sources: &SourceSnapshot,
    documents: &DocumentMap,
    encoding: PositionEncoding,
) -> Result<PublishDiagnosticsParams, LspError> {
    let main_index = main_label_index(diagnostic)?;

    let main_label = &diagnostic.labels[main_index];

    let document =
        documents
            .get(&main_label.location.file)
            .ok_or_else(|| LspError::MissingDocument {
                source: main_label.location.file.clone(),
            })?;

    let lsp = to_lsp_diagnostic(diagnostic, sources, documents, encoding)?;

    Ok(PublishDiagnosticsParams::new(
        document.uri.clone(),
        vec![lsp],
        document.version,
    ))
}

pub(crate) fn main_label_index(diagnostic: &CoreDiagnostic) -> Result<usize, LspError> {
    diagnostic
        .labels
        .iter()
        .position(|label| label.kind == LabelKind::Primary)
        .or_else(|| (!diagnostic.labels.is_empty()).then_some(0))
        .ok_or(LspError::MissingLabel)
}

fn lsp_severity(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Trace | Severity::Debug => DiagnosticSeverity::HINT,

        Severity::Info => DiagnosticSeverity::INFORMATION,

        Severity::Warning => DiagnosticSeverity::WARNING,

        Severity::Error | Severity::Fatal => DiagnosticSeverity::ERROR,
    }
}

fn related_message(label: &Label) -> String {
    label.message.clone().unwrap_or_else(|| match label.kind {
        LabelKind::Primary => "additional primary location".to_owned(),

        LabelKind::Secondary => "related location".to_owned(),
    })
}

fn diagnostic_message(diagnostic: &CoreDiagnostic) -> String {
    let mut message = diagnostic.message.clone();

    for note in &diagnostic.notes {
        message.push_str("\nnote: ");
        message.push_str(note);
    }

    if let Some(help) = &diagnostic.help {
        message.push_str("\nhelp: ");
        message.push_str(help);
    }

    if let Some(cause) = &diagnostic.cause {
        for cause in cause.iter() {
            message.push_str("\ncaused by: ");

            message.push_str(&cause.message);
        }
    }

    message
}
