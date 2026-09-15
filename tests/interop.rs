use diagprint::{
    Applicability, Cause, DocumentationLink, InteropDiagnostic, InteropDiagnosticSource,
    InteropDiagnosticSourceExt, InteropLabel, LabelKind, Reporter, Severity,
};

struct Producer;

impl InteropDiagnosticSource for Producer {
    fn to_interop_diagnostic(&self) -> InteropDiagnostic {
        InteropDiagnostic::new("external diagnostic")
            .severity(Severity::Warning)
            .code("EXT-001")
            .help("review the external diagnostic")
            .note("structured note")
            .label(
                InteropLabel::primary("src/main.rs", 10)
                    .column(4)
                    .length(5)
                    .message("primary location"),
            )
            .label(
                InteropLabel::secondary("src/lib.rs", 20)
                    .column(2)
                    .length(3)
                    .message("related location"),
            )
            .cause(Cause::new("underlying failure"))
            .documentation(DocumentationLink::new(
                "external docs",
                "https://example.com/docs",
            ))
            .related(InteropDiagnostic::new("related warning").severity(Severity::Info))
    }
}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("interop-tests")
        .build()
        .unwrap()
}

#[test]
fn custom_source_converts_through_generic_protocol() {
    let diagnostic = Producer.to_diagprint(&reporter());

    assert_eq!(diagnostic.message, "external diagnostic");

    assert_eq!(diagnostic.severity, Severity::Warning);

    assert_eq!(diagnostic.code.as_deref(), Some("EXT-001"));

    assert_eq!(
        diagnostic.help.as_deref(),
        Some("review the external diagnostic")
    );

    assert_eq!(diagnostic.labels.len(), 2);

    assert_eq!(diagnostic.labels[0].kind, LabelKind::Primary);

    assert_eq!(diagnostic.labels[1].kind, LabelKind::Secondary);

    assert_eq!(diagnostic.labels[1].location.file, "src/lib.rs");

    assert_eq!(
        diagnostic
            .cause
            .as_ref()
            .map(|cause| { cause.message.as_str() },),
        Some("underlying failure")
    );
}

#[test]
fn documentation_never_becomes_automatic_remediation() {
    let diagnostic = Producer.to_diagprint(&reporter());

    assert_eq!(diagnostic.suggestions.len(), 1);

    let suggestion = &diagnostic.suggestions[0];

    assert_eq!(suggestion.applicability, Applicability::Manual);

    assert!(suggestion.edits.is_empty());

    assert!(suggestion.commands.is_empty());

    assert_eq!(suggestion.documentation.len(), 1);
}

#[test]
fn related_diagnostics_remain_structured() {
    let tree = Producer.to_diagprint_tree(&reporter());

    assert_eq!(tree.related_count(), 1);

    assert_eq!(tree.total_diagnostics(), 2);

    assert_eq!(tree.related[0].diagnostic.message, "related warning");

    assert_eq!(tree.related[0].diagnostic.severity, Severity::Info);

    assert!(
        tree.diagnostic
            .notes
            .iter()
            .any(|note| { note == "related diagnostics: 1" },)
    );
}

#[test]
fn direct_interop_diagnostic_can_convert_without_trait() {
    let diagnostic = InteropDiagnostic::new("direct")
        .severity(Severity::Debug)
        .to_diagprint(&reporter());

    assert_eq!(diagnostic.message, "direct");

    assert_eq!(diagnostic.severity, Severity::Debug);

    assert!(diagnostic.suggestions.is_empty());
}
