use diagprint::{Applicability, Edit, Reporter, Suggestion, TextRange};
use diagprint_lsp::{
    DocumentMap, LspAdapter, LspError, PositionEncoding,
    lsp_types::{DiagnosticSeverity, DocumentChanges, Uri},
};
use std::str::FromStr;

fn uri(name: &str) -> Uri {
    Uri::from_str(&format!("file:///workspace/{name}")).unwrap()
}

fn documents(names: &[(&str, i32)]) -> DocumentMap {
    let mut documents = DocumentMap::new();

    for (name, version) in names {
        documents.insert_versioned(*name, uri(name), *version);
    }

    documents
}

#[test]
fn diagnostic_maps_primary_and_related_locations() {
    let reporter = Reporter::builder().application("lsp-test").build().unwrap();

    let main_revision = reporter
        .source_cache()
        .insert_revisioned("main.rs", "let broken = value;\n");

    let other_revision = reporter
        .source_cache()
        .insert_revisioned("other.rs", "let value = 1;\n");

    let diagnostic = reporter
        .error("invalid value")
        .code("E-LSP-1")
        .label_at_revision(
            "main.rs",
            main_revision,
            1,
            Some(5),
            Some(6),
            Some("broken value"),
        )
        .secondary_label_at_revision(
            "other.rs",
            other_revision,
            1,
            Some(5),
            Some(5),
            Some("declared here"),
        );

    let captured = reporter.capture(diagnostic);

    let adapter = LspAdapter::utf16(documents(&[("main.rs", 7), ("other.rs", 3)]));

    let lsp = adapter.captured_diagnostic(&captured).unwrap();

    assert_eq!(lsp.severity, Some(DiagnosticSeverity::ERROR));

    assert_eq!(lsp.range.start.line, 0);

    assert_eq!(lsp.range.start.character, 4);

    assert_eq!(lsp.related_information.as_ref().unwrap().len(), 1);
}

#[test]
fn utf_encodings_are_converted_correctly() {
    let reporter = Reporter::builder()
        .application("encoding-test")
        .build()
        .unwrap();

    let revision = reporter
        .source_cache()
        .insert_revisioned("emoji.rs", "a😀b\n");

    let diagnostic = reporter.error("emoji").label_at_revision(
        "emoji.rs",
        revision,
        1,
        Some(2),
        Some(1),
        Some("emoji"),
    );

    let captured = reporter.capture(diagnostic);

    let docs = documents(&[("emoji.rs", 1)]);

    let utf8 = LspAdapter::new(docs.clone(), PositionEncoding::Utf8)
        .captured_diagnostic(&captured)
        .unwrap();

    assert_eq!(utf8.range.start.character, 1);

    assert_eq!(utf8.range.end.character, 5);

    let utf16 = LspAdapter::new(docs.clone(), PositionEncoding::Utf16)
        .captured_diagnostic(&captured)
        .unwrap();

    assert_eq!(utf16.range.start.character, 1);

    assert_eq!(utf16.range.end.character, 3);

    let utf32 = LspAdapter::new(docs, PositionEncoding::Utf32)
        .captured_diagnostic(&captured)
        .unwrap();

    assert_eq!(utf32.range.start.character, 1);

    assert_eq!(utf32.range.end.character, 2);
}

#[test]
fn source_revision_mismatch_fails_closed() {
    let reporter = Reporter::builder()
        .application("revision-test")
        .build()
        .unwrap();

    let old_revision = reporter
        .source_cache()
        .insert_revisioned("main.rs", "old\n");

    reporter
        .source_cache()
        .insert_revisioned("main.rs", "new\n");

    let diagnostic = reporter.error("stale").label_at_revision(
        "main.rs",
        old_revision,
        1,
        Some(1),
        Some(3),
        Some("old source"),
    );

    let captured = reporter.capture(diagnostic);

    let adapter = LspAdapter::utf16(documents(&[("main.rs", 2)]));

    let error = adapter.captured_diagnostic(&captured).unwrap_err();

    assert!(matches!(error, LspError::RevisionMismatch { .. }));
}

#[test]
fn machine_fix_becomes_versioned_workspace_edit() {
    let reporter = Reporter::builder()
        .application("action-test")
        .build()
        .unwrap();

    let revision = reporter
        .source_cache()
        .insert_revisioned("main.rs", "let old = 1;\n");

    let suggestion = Suggestion::new("Replace old with new")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace("main.rs", TextRange::new(4, 7), "old", "new"));

    let diagnostic = reporter
        .error("old name")
        .label_at_revision("main.rs", revision, 1, Some(5), Some(3), Some("old"))
        .suggestion(suggestion);

    let captured = reporter.capture(diagnostic);

    let adapter = LspAdapter::utf16(documents(&[("main.rs", 42)]));

    let actions = adapter.code_actions(&captured).unwrap();

    assert_eq!(actions.len(), 1);

    let action = &actions[0];

    assert_eq!(action.is_preferred, Some(true));

    assert!(action.disabled.is_none());

    let edit = action.edit.as_ref().unwrap();

    let Some(DocumentChanges::Edits(edits)) = &edit.document_changes else {
        panic!("expected versioned document edits");
    };

    assert_eq!(edits.len(), 1);

    assert_eq!(edits[0].text_document.version, Some(42));
}

#[test]
fn stale_guard_becomes_disabled_action() {
    let reporter = Reporter::builder()
        .application("guard-test")
        .build()
        .unwrap();

    let revision = reporter
        .source_cache()
        .insert_revisioned("main.rs", "let old = 1;\n");

    let suggestion = Suggestion::new("bad fix")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "main.rs",
            TextRange::new(4, 7),
            "not-old",
            "new",
        ));

    let captured = reporter.capture(
        reporter
            .error("bad fix")
            .label_at_revision("main.rs", revision, 1, Some(5), Some(3), Some("old"))
            .suggestion(suggestion),
    );

    let adapter = LspAdapter::utf16(documents(&[("main.rs", 1)]));

    let actions = adapter.code_actions(&captured).unwrap();

    assert_eq!(actions.len(), 1);

    assert!(actions[0].edit.is_none());

    assert!(actions[0].disabled.is_some());
}

#[test]
fn manual_suggestion_never_becomes_edit() {
    let reporter = Reporter::builder()
        .application("manual-test")
        .build()
        .unwrap();

    let revision = reporter
        .source_cache()
        .insert_revisioned("main.rs", "value\n");

    let captured = reporter.capture(
        reporter
            .warning("review")
            .label_at_revision("main.rs", revision, 1, Some(1), Some(5), Some("review"))
            .suggestion(Suggestion::new("Review manually")),
    );

    let adapter = LspAdapter::utf16(documents(&[("main.rs", 9)]));

    let actions = adapter.code_actions(&captured).unwrap();

    assert_eq!(actions.len(), 1);

    assert!(actions[0].edit.is_none());

    assert!(actions[0].disabled.is_some());
}

#[test]
fn publish_uses_external_document_version() {
    let reporter = Reporter::builder()
        .application("publish-test")
        .build()
        .unwrap();

    let revision = reporter
        .source_cache()
        .insert_revisioned("main.rs", "bad\n");

    let captured = reporter.capture(reporter.error("bad").label_at_revision(
        "main.rs",
        revision,
        1,
        Some(1),
        Some(3),
        Some("bad"),
    ));

    let adapter = LspAdapter::utf16(documents(&[("main.rs", 123)]));

    let publish = adapter.publish_captured(&captured).unwrap();

    assert_eq!(publish.version, Some(123));

    assert_eq!(revision.get(), 1);
}
