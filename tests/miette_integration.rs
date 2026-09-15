#![cfg(feature = "miette")]

use diagprint::{MietteDiagnosticExt, MietteReportExt, Reporter, Severity as DiagSeverity};
use miette::{
    Diagnostic as MietteDiagnostic, LabeledSpan, MietteDiagnostic as DynamicDiagnostic,
    NamedSource, Report, Severity as MietteSeverity,
};
use std::{error::Error, fmt};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("miette-tests")
        .build()
        .unwrap()
}

#[test]
fn basic_metadata_is_preserved() {
    let source = DynamicDiagnostic::new("configuration is suspicious")
        .with_code("demo::config")
        .with_severity(MietteSeverity::Warning)
        .with_help("check the configuration")
        .with_url("https://example.com/config");

    let diagnostic = source.to_diagprint(&reporter());

    assert_eq!(diagnostic.message, "configuration is suspicious");

    assert_eq!(diagnostic.severity, DiagSeverity::Warning);

    assert_eq!(diagnostic.code.as_deref(), Some("demo::config"));

    assert_eq!(diagnostic.help.as_deref(), Some("check the configuration"));

    assert_eq!(diagnostic.suggestions.len(), 1);

    assert_eq!(
        diagnostic.suggestions[0].documentation[0].url,
        "https://example.com/config"
    );

    assert!(!diagnostic.suggestions[0].is_machine_applicable());

    assert!(diagnostic.suggestions[0].edits.is_empty());
}

#[test]
fn missing_miette_severity_defaults_to_error() {
    let source = DynamicDiagnostic::new("default severity");

    let diagnostic = source.to_diagprint(&reporter());

    assert_eq!(diagnostic.severity, DiagSeverity::Error);
}

#[test]
fn advice_maps_to_info() {
    let source = DynamicDiagnostic::new("consider this").with_severity(MietteSeverity::Advice);

    let diagnostic = source.to_diagprint(&reporter());

    assert_eq!(diagnostic.severity, DiagSeverity::Info);
}

#[test]
fn report_source_labels_are_resolved() {
    let source = "fn main() {\n    let answer = 41;\n}\n";

    let offset = source.find("41").unwrap();

    let diagnostic = DynamicDiagnostic::new("wrong answer").with_label(LabeledSpan::new(
        Some("expected 42".into()),
        offset,
        2,
    ));

    let report = Report::new(diagnostic)
        .with_source_code(NamedSource::new("src/main.rs", source.to_owned()));

    let converted = report.to_diagprint(&reporter());

    assert_eq!(converted.labels.len(), 1);

    let label = &converted.labels[0];

    assert_eq!(label.location.file, "src/main.rs");

    assert_eq!(label.location.line, 2);

    assert_eq!(label.location.column, Some(18));

    assert_eq!(label.length, Some(2));

    assert_eq!(label.message.as_deref(), Some("expected 42"));
}

#[test]
fn labels_without_source_code_are_not_silently_lost() {
    let source = DynamicDiagnostic::new("missing source").with_label(LabeledSpan::new(
        Some("somewhere".into()),
        4,
        3,
    ));

    let diagnostic = source.to_diagprint(&reporter());

    assert!(diagnostic.labels.is_empty());

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note.contains("miette source span bytes 4..7") },)
    );
}

#[derive(Debug)]
struct LeafCause;

impl fmt::Display for LeafCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "leaf cause")
    }
}

impl Error for LeafCause {}

#[derive(Debug)]
struct RootCause {
    source: LeafCause,
}

impl fmt::Display for RootCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root diagnostic")
    }
}

impl Error for RootCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

impl MietteDiagnostic for RootCause {}

#[test]
fn standard_error_source_chain_is_preserved() {
    let source = RootCause { source: LeafCause };

    let diagnostic = source.to_diagprint(&reporter());

    let cause = diagnostic.cause.expect("cause chain");

    assert_eq!(cause.message, "leaf cause");
}

#[derive(Debug)]
struct RelatedRoot {
    related: Vec<DynamicDiagnostic>,
}

impl fmt::Display for RelatedRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root with related diagnostics")
    }
}

impl Error for RelatedRoot {}

impl MietteDiagnostic for RelatedRoot {
    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn MietteDiagnostic> + 'a>> {
        Some(Box::new(
            self.related
                .iter()
                .map(|diagnostic| diagnostic as &dyn MietteDiagnostic),
        ))
    }
}

#[test]
fn related_diagnostics_remain_structured() {
    let root = RelatedRoot {
        related: vec![
            DynamicDiagnostic::new("first related").with_severity(MietteSeverity::Warning),
            DynamicDiagnostic::new("second related"),
        ],
    };

    let tree = root.to_diagprint_tree(&reporter());

    assert_eq!(tree.related_count(), 2);

    assert_eq!(tree.total_diagnostics(), 3);

    assert_eq!(tree.related[0].diagnostic.message, "first related");

    assert_eq!(tree.related[0].diagnostic.severity, DiagSeverity::Warning);

    assert_eq!(tree.related[1].diagnostic.message, "second related");

    assert!(
        tree.diagnostic
            .notes
            .iter()
            .any(|note| { note == "miette related diagnostics: 2" },)
    );
}
