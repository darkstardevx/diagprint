#![cfg(feature = "ariadne")]

use diagprint::{
    AriadneBridge, AriadneBridgeError, AriadneBridgeLabel, LabelKind, Reporter, Severity,
};

#[test]
fn bridge_preserves_structured_diagnostic_metadata() {
    let bridge = AriadneBridge::new(Severity::Error, "Incompatible types", "sample.tao", 2..3)
        .source("sample.tao", "a β c\nsecond line\n")
        .code("E-ARIADNE")
        .help("use matching types")
        .note("type checking stopped here")
        .label(AriadneBridgeLabel::primary("sample.tao", 2..3).message("this is the primary value"))
        .label(
            AriadneBridgeLabel::secondary("sample.tao", 4..5)
                .message("this is related")
                .order(2)
                .priority(1),
        );

    let interop = bridge.to_interop_diagnostic();

    assert_eq!(interop.severity, Severity::Error);
    assert_eq!(interop.code.as_deref(), Some("E-ARIADNE"));
    assert_eq!(interop.help.as_deref(), Some("use matching types"));

    assert!(
        interop
            .notes
            .iter()
            .any(|note| note == "type checking stopped here")
    );

    assert_eq!(interop.labels.len(), 2);

    assert_eq!(interop.labels[0].kind, LabelKind::Primary);
    assert_eq!(interop.labels[0].file, "sample.tao");
    assert_eq!(interop.labels[0].line, 1);
    assert_eq!(interop.labels[0].column, Some(3));
    assert_eq!(interop.labels[0].length, Some(1));

    assert_eq!(interop.labels[1].kind, LabelKind::Secondary);
    assert_eq!(interop.labels[1].column, Some(5));
}

#[test]
fn bridge_emits_a_real_ariadne_report() {
    let bridge = AriadneBridge::new(Severity::Error, "Incompatible types", "sample.tao", 2..3)
        .source("sample.tao", "a b c\n")
        .code("E-ARIADNE")
        .primary_label("sample.tao", 2..3, "primary value")
        .secondary_label("sample.tao", 4..5, "related value");

    let report = bridge.to_ariadne_report().unwrap();

    let mut output = Vec::new();

    report
        .write(ariadne::sources(bridge.ariadne_sources()), &mut output)
        .unwrap();

    let output = String::from_utf8(output).unwrap();

    assert!(output.contains("Incompatible types"));
    assert!(output.contains("primary value"));
    assert!(output.contains("related value"));
}

#[test]
fn character_offsets_resolve_to_one_based_lines_and_columns() {
    let bridge = AriadneBridge::new(Severity::Warning, "unicode spans", "unicode.rs", 1..2)
        .source("unicode.rs", "αβ\nhello\n")
        .primary_label("unicode.rs", 1..2, "beta")
        .secondary_label("unicode.rs", 3..8, "hello");

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
fn missing_sources_fail_closed_for_ariadne_and_become_interop_notes() {
    let bridge = AriadneBridge::new(Severity::Error, "missing source", "missing.rs", 0..1);

    assert!(matches!(
        bridge.to_ariadne_report(),
        Err(AriadneBridgeError::MissingSource { .. })
    ));

    let interop = bridge.to_interop_diagnostic();

    assert!(interop.labels.is_empty());

    assert!(interop.notes.iter().any(|note| {
        note.contains("Ariadne report span could not be resolved") && note.contains("missing.rs")
    }));
}

#[test]
fn bridge_converts_directly_to_diagprint() {
    let reporter = Reporter::builder()
        .application("ariadne-test")
        .build()
        .unwrap();

    let bridge = AriadneBridge::new(Severity::Error, "bridge diagnostic", "bridge.rs", 0..3)
        .source("bridge.rs", "let value = 1;\n")
        .primary_label("bridge.rs", 0..3, "primary");

    let diagnostic = bridge.to_diagprint(&reporter);

    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(diagnostic.labels[0].kind, LabelKind::Primary);
}
