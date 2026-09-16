use diagprint::{
    DiagnosticReport, Reporter,
    render::{AuditTranscriptRenderer, CompilerTextRenderer, RenderedFormat, Renderer},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("text-renderer-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

#[test]
fn compiler_text_uses_editor_friendly_location_grammar() {
    let diagnostic = reporter()
        .error("expected integer, found string")
        .code("E0308")
        .label("src/main.rs", 42, Some(17), Some(5), Some("wrong type"));

    let output = CompilerTextRenderer.render(&diagnostic);

    assert_eq!(
        output,
        "src/main.rs:42:17: error[E0308]: expected integer, found string",
    );
}

#[test]
fn compiler_text_removes_embedded_newlines() {
    let diagnostic = reporter().warning("first line\nsecond line").code("W100");

    let output = CompilerTextRenderer.render(&diagnostic);

    assert_eq!(
        output,
        "<unknown>:0:0: warning[W100]: first line second line",
    );

    assert!(!output.contains('\n'));
}

#[test]
fn audit_transcript_excludes_volatile_runtime_metadata() {
    let report = DiagnosticReport::from_diagnostic(reporter().error("failure").code("E100").label(
        "src/lib.rs",
        10,
        Some(4),
        Some(3),
        Some("target"),
    ));

    let output = AuditTranscriptRenderer
        .render_report(&report)
        .expect("audit transcript should render");

    assert!(output.starts_with("DIAGPRINT AUDIT TRANSCRIPT v1\n"));

    assert!(output.contains("report-digest:"));
    assert!(output.contains("fingerprint:"));
    assert!(output.contains("severity: error"));
    assert!(output.contains("code: E100"));

    assert!(!output.contains("timestamp:"));
    assert!(!output.contains("session:"));
    assert!(!output.contains("report-id:"));
    assert!(!output.contains("pid:"));
    assert!(!output.contains("hostname:"));
}

#[test]
fn audit_transcript_is_independent_of_report_insertion_order() {
    let reporter = reporter();

    let warning = reporter.warning("warning").code("W200");

    let error = reporter.error("error").code("E200");

    let mut first = DiagnosticReport::new();
    first.push(warning.clone()).push(error.clone());

    let mut second = DiagnosticReport::new();
    second.push(error).push(warning);

    let first_output = AuditTranscriptRenderer
        .render_report(&first)
        .expect("first transcript should render");

    let second_output = AuditTranscriptRenderer
        .render_report(&second)
        .expect("second transcript should render");

    assert_eq!(first_output, second_output);
}

#[test]
fn compiler_and_audit_artifacts_share_semantic_identity() {
    let report = DiagnosticReport::from_diagnostic(reporter().error("failure").code("E300"));

    let compiler = CompilerTextRenderer
        .render_report_artifact(&report)
        .expect("compiler artifact should render");

    let audit = AuditTranscriptRenderer
        .render_report_artifact(&report)
        .expect("audit artifact should render");

    assert_eq!(compiler.receipt().report, audit.receipt().report,);

    assert_ne!(
        compiler.receipt().artifact_digest,
        audit.receipt().artifact_digest,
    );

    assert_eq!(compiler.format(), RenderedFormat::CompilerText,);

    assert_eq!(audit.format(), RenderedFormat::AuditTranscript,);
}

#[test]
fn new_rendered_format_metadata_is_stable() {
    assert_eq!(RenderedFormat::CompilerText.as_str(), "compiler_text",);

    assert_eq!(
        RenderedFormat::CompilerText.artifact_schema(),
        "diagprint.report.compiler-text/v1",
    );

    assert_eq!(
        RenderedFormat::CompilerText.media_type(),
        "text/plain; charset=utf-8",
    );

    assert_eq!(RenderedFormat::AuditTranscript.as_str(), "audit_transcript",);

    assert_eq!(
        RenderedFormat::AuditTranscript.artifact_schema(),
        "diagprint.report.audit/v1",
    );

    assert_eq!(RenderedFormat::AuditTranscript.extension(), "audit",);
}
