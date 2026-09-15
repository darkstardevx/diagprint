#![cfg(feature = "annotate-snippets")]

use annotate_snippets::Renderer;
use diagprint::{
    AnnotateSnippetsBridge, AnnotateSnippetsBridgeError, LabelKind, Reporter, Severity,
    render::TerminalRenderer,
};

#[test]
fn bridge_preserves_primary_and_secondary_labels() {
    let bridge = AnnotateSnippetsBridge::new(Severity::Error, "mismatched values")
        .source("sample.rs", "let left = 42;\nlet right = \"forty-two\";\n")
        .code("E-SNIPPET")
        .help("make both values use the same type")
        .note("comparison happened here")
        .primary_label("sample.rs", 4..8, "numeric value")
        .secondary_label("sample.rs", 28..37, "string value");

    let interop = bridge.to_interop_diagnostic();

    assert_eq!(interop.severity, Severity::Error);
    assert_eq!(interop.code.as_deref(), Some("E-SNIPPET"));
    assert_eq!(
        interop.help.as_deref(),
        Some("make both values use the same type")
    );

    assert_eq!(interop.labels.len(), 2);

    assert_eq!(interop.labels[0].kind, LabelKind::Primary);
    assert_eq!(interop.labels[0].line, 1);
    assert_eq!(interop.labels[0].column, Some(5));
    assert_eq!(interop.labels[0].length, Some(4));

    assert_eq!(interop.labels[1].kind, LabelKind::Secondary);
    assert_eq!(interop.labels[1].line, 2);
    assert_eq!(interop.labels[1].column, Some(14));
    assert_eq!(interop.labels[1].length, Some(9));
}

#[test]
fn bridge_emits_real_annotate_snippets_groups() {
    let bridge = AnnotateSnippetsBridge::new(Severity::Error, "mismatched values")
        .source("sample.rs", "let left = 42;\nlet right = \"forty-two\";\n")
        .code("E-SNIPPET")
        .help("make both values use the same type")
        .primary_label("sample.rs", 4..8, "numeric value")
        .secondary_label("sample.rs", 28..37, "string value");

    let groups = bridge.to_annotate_snippets_groups().unwrap();

    let rendered = Renderer::plain().render(&groups);

    assert!(rendered.contains("mismatched values"));
    assert!(rendered.contains("E-SNIPPET"));
    assert!(rendered.contains("numeric value"));
    assert!(rendered.contains("string value"));
    assert!(rendered.contains("make both values use the same type"));
}

#[test]
fn byte_ranges_become_character_columns_and_lengths() {
    let bridge = AnnotateSnippetsBridge::new(Severity::Warning, "unicode spans")
        .source("unicode.rs", "αβ\nhello\n")
        .primary_label("unicode.rs", 2..4, "beta")
        .secondary_label("unicode.rs", 5..10, "hello");

    let interop = bridge.to_interop_diagnostic();

    assert_eq!(interop.labels.len(), 2);

    assert_eq!(interop.labels[0].line, 1);
    assert_eq!(interop.labels[0].column, Some(2));
    assert_eq!(interop.labels[0].length, Some(1));

    assert_eq!(interop.labels[1].line, 2);
    assert_eq!(interop.labels[1].column, Some(1));
    assert_eq!(interop.labels[1].length, Some(5));
}

#[test]
fn invalid_utf8_boundary_fails_closed() {
    let bridge = AnnotateSnippetsBridge::new(Severity::Error, "invalid span")
        .source("unicode.rs", "αβ\n")
        .primary_label("unicode.rs", 1..2, "inside alpha");

    assert!(matches!(
        bridge.to_annotate_snippets_groups(),
        Err(AnnotateSnippetsBridgeError::InvalidUtf8Boundary { .. })
    ));

    let interop = bridge.to_interop_diagnostic();

    assert!(interop.labels.is_empty());

    assert!(
        interop
            .notes
            .iter()
            .any(|note| { note.contains("annotate-snippets source label could not be resolved") })
    );
}

#[test]
fn bridge_converts_directly_to_diagprint() {
    let reporter = Reporter::builder()
        .application("annotate-snippets-test")
        .build()
        .unwrap();

    let bridge = AnnotateSnippetsBridge::new(Severity::Error, "bridge diagnostic")
        .source("bridge.rs", "let value = 1;\n")
        .primary_label("bridge.rs", 4..9, "value");

    let diagnostic = bridge.to_diagprint(&reporter);

    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(diagnostic.labels[0].kind, LabelKind::Primary);
}

#[test]
fn bridge_sources_feed_virtual_diagprint_rendering() {
    const SOURCE_NAME: &str = "memory://annotate-snippets/example.rs";

    let bridge =
        AnnotateSnippetsBridge::new(Severity::Error, "virtual annotate-snippets diagnostic")
            .source(SOURCE_NAME, "let left = 42;\nlet right = \"forty-two\";\n")
            .primary_label(SOURCE_NAME, 4..8, "numeric value")
            .secondary_label(SOURCE_NAME, 28..37, "string value");

    let reporter = Reporter::builder()
        .application("annotate-snippets-virtual-source-test")
        .sources_from(&bridge)
        .build()
        .unwrap();

    let diagnostic = bridge.to_diagprint(&reporter);

    let renderer = TerminalRenderer {
        color: false,
        width: 72,
        ..Default::default()
    };

    let cache = reporter.source_cache();

    let rendered = renderer.render_with_sources(&diagnostic, &cache);

    assert!(rendered.contains("memory://annotate-snippets/example.rs"));

    assert!(rendered.contains("let left = 42;"));
    assert!(rendered.contains("let right = \"forty-two\";"));
    assert!(rendered.contains("^^^^"));
    assert!(rendered.contains("---------"));
    assert!(rendered.contains("└─ numeric value"));
    assert!(rendered.contains("└· string value"));
}
