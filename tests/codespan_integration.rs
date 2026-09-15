#![cfg(feature = "codespan-reporting")]

use codespan_reporting::{
    diagnostic::{Diagnostic, Label, Severity as CodespanSeverity},
    files::SimpleFiles,
};
use diagprint::{CodespanDiagnosticExt, Reporter, Severity};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("codespan-tests")
        .build()
        .unwrap()
}

#[test]
fn structured_metadata_is_preserved() {
    let mut files = SimpleFiles::new();

    let file = files.add("src/main.rs", "fn main() {}\n");

    let source = Diagnostic {
        severity: CodespanSeverity::Warning,

        code: Some("W100".into()),

        message: "test warning".into(),

        labels: vec![Label::primary(file, 3..7).with_message("main function")],

        notes: vec!["first note".into(), "second note".into()],
    };

    let diagnostic = source.to_diagprint(&reporter(), &files);

    assert_eq!(diagnostic.severity, Severity::Warning);

    assert_eq!(diagnostic.code.as_deref(), Some("W100"));

    assert_eq!(diagnostic.message, "test warning");

    assert_eq!(diagnostic.notes, ["first note", "second note",]);

    assert!(diagnostic.suggestions.is_empty());
}

#[test]
fn source_position_is_resolved_through_files_database() {
    let source = "fn main() {\n    let value = wrong;\n}\n";

    let offset = source.find("wrong").unwrap();

    let mut files = SimpleFiles::new();

    let file = files.add("src/main.rs", source);

    let codespan = Diagnostic {
        severity: CodespanSeverity::Error,

        code: None,

        message: "unknown value".into(),

        labels: vec![Label::primary(file, offset..offset + 5).with_message("not defined")],

        notes: Vec::new(),
    };

    let diagnostic = codespan.to_diagprint(&reporter(), &files);

    assert_eq!(diagnostic.labels.len(), 1);

    let label = &diagnostic.labels[0];

    assert_eq!(label.location.file, "src/main.rs");

    assert_eq!(label.location.line, 2);

    assert_eq!(label.location.column, Some(17));

    assert_eq!(label.length, Some(5));

    assert_eq!(label.message.as_deref(), Some("not defined"));
}

#[test]
fn codespan_severity_mapping_is_conservative() {
    let mut files = SimpleFiles::new();

    let file = files.add("demo.rs", "x");

    let cases = [
        (CodespanSeverity::Bug, Severity::Fatal),
        (CodespanSeverity::Error, Severity::Error),
        (CodespanSeverity::Warning, Severity::Warning),
        (CodespanSeverity::Note, Severity::Info),
        (CodespanSeverity::Help, Severity::Info),
    ];

    for (source_severity, expected) in cases {
        let source = Diagnostic {
            severity: source_severity,

            code: None,

            message: "severity test".into(),

            labels: vec![Label::primary(file, 0..1)],

            notes: Vec::new(),
        };

        assert_eq!(source.to_diagprint(&reporter(), &files,).severity, expected);
    }
}

#[test]
fn secondary_labels_are_retained_and_declared() {
    let mut files = SimpleFiles::new();

    let file = files.add("demo.rs", "abcdef");

    let source = Diagnostic {
        severity: CodespanSeverity::Error,

        code: None,

        message: "two locations".into(),

        labels: vec![
            Label::primary(file, 0..1).with_message("primary"),
            Label::secondary(file, 2..3).with_message("context"),
        ],

        notes: Vec::new(),
    };

    let diagnostic = source.to_diagprint(&reporter(), &files);

    assert_eq!(diagnostic.labels.len(), 2);

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note == "codespan secondary source labels: 1" },)
    );
}

#[test]
fn multiline_span_does_not_fake_single_line_width() {
    let source = "first\nsecond\n";

    let mut files = SimpleFiles::new();

    let file = files.add("demo.rs", source);

    let diagnostic = Diagnostic {
        severity: CodespanSeverity::Error,

        code: None,

        message: "multiline".into(),

        labels: vec![Label::primary(file, 0..source.len())],

        notes: Vec::new(),
    }
    .to_diagprint(&reporter(), &files);

    assert_eq!(diagnostic.labels.len(), 1);

    assert_eq!(diagnostic.labels[0].length, None);
}

#[test]
fn invalid_source_range_degrades_to_note() {
    let mut files = SimpleFiles::new();

    let file = files.add("demo.rs", "abc");

    let diagnostic = Diagnostic {
        severity: CodespanSeverity::Error,

        code: None,

        message: "bad span".into(),

        labels: vec![Label::primary(file, 0..100).with_message("invalid")],

        notes: Vec::new(),
    }
    .to_diagprint(&reporter(), &files);

    assert!(diagnostic.labels.is_empty());

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note.contains("codespan primary label bytes 0..100") },)
    );
}

#[test]
fn unicode_column_and_width_are_preserved() {
    let source = "αβ wrong\n";

    let offset = source.find("wrong").unwrap();

    let mut files = SimpleFiles::new();

    let file = files.add("unicode.rs", source);

    let diagnostic = Diagnostic {
        severity: CodespanSeverity::Error,

        code: None,

        message: "unicode".into(),

        labels: vec![Label::primary(file, offset..offset + 5)],

        notes: Vec::new(),
    }
    .to_diagprint(&reporter(), &files);

    let label = &diagnostic.labels[0];

    assert_eq!(label.location.line, 1);

    assert_eq!(label.location.column, Some(4));

    assert_eq!(label.length, Some(5));
}
