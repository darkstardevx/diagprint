use crate::{
    DocumentMap, LspError, PositionEncoding, action,
    diagnostic::{main_label_index, to_lsp_diagnostic},
};
use diagprint::{DiagnosticReport, SourceSnapshot};
use lsp_types::{CodeAction, Diagnostic as LspDiagnostic, PublishDiagnosticsParams};
use std::collections::BTreeMap;

pub(crate) fn publish_report(
    report: &DiagnosticReport,
    sources: &SourceSnapshot,
    documents: &DocumentMap,
    encoding: PositionEncoding,
) -> Result<Vec<PublishDiagnosticsParams>, LspError> {
    let mut grouped: BTreeMap<String, Vec<LspDiagnostic>> = BTreeMap::new();

    for diagnostic in report {
        let main_index = main_label_index(diagnostic)?;

        let source_name = diagnostic.labels[main_index].location.file.clone();

        let lsp = to_lsp_diagnostic(diagnostic, sources, documents, encoding)?;

        grouped.entry(source_name).or_default().push(lsp);
    }

    let mut published = Vec::with_capacity(grouped.len());

    for (source_name, diagnostics) in grouped {
        let document = documents
            .get(&source_name)
            .ok_or_else(|| LspError::MissingDocument {
                source: source_name.clone(),
            })?;

        published.push(PublishDiagnosticsParams::new(
            document.uri.clone(),
            diagnostics,
            document.version,
        ));
    }

    Ok(published)
}

pub(crate) fn code_actions_report(
    report: &DiagnosticReport,
    sources: &SourceSnapshot,
    documents: &DocumentMap,
    encoding: PositionEncoding,
) -> Result<Vec<CodeAction>, LspError> {
    let mut actions = Vec::new();

    for diagnostic in report {
        actions.extend(action::code_actions(
            diagnostic, sources, documents, encoding,
        )?);
    }

    Ok(actions)
}
