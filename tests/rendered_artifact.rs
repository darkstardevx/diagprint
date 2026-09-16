use diagprint::{
    ArtifactVerificationError, ArtifactWriteError, ArtifactWriter, DiagnosticReport, Reporter,
    SourceCache,
    render::{
        MarkdownRenderer, PlainRenderer, RENDERED_RECEIPT_V1_SCHEMA, RenderedFormat,
        RenderedSourceMode,
    },
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "html")]
use diagprint::render::HtmlRenderer;

fn reporter() -> Reporter {
    Reporter::builder()
        .application("rendered-artifact-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

fn sample_report() -> DiagnosticReport {
    let reporter = reporter();

    let mut report = DiagnosticReport::new();

    report
        .push(reporter.warning("unused binding").code("lint::unused"))
        .push(reporter.error("parse failed").code("parse::failed"));

    report
}

fn temporary_destination(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!("diagprint-{name}-{}-{nonce}", std::process::id(),))
}

#[test]
fn rendered_format_metadata_is_stable() {
    assert_eq!(RenderedFormat::Html.as_str(), "html");
    assert_eq!(
        RenderedFormat::Html.artifact_schema(),
        "diagprint.report.html/v1",
    );
    assert_eq!(
        RenderedFormat::Html.media_type(),
        "text/html; charset=utf-8",
    );
    assert_eq!(RenderedFormat::Html.extension(), "html");

    assert_eq!(
        RenderedFormat::Markdown.artifact_schema(),
        "diagprint.report.markdown/v1",
    );
    assert_eq!(
        RenderedFormat::Markdown.media_type(),
        "text/markdown; charset=utf-8",
    );
    assert_eq!(RenderedFormat::Markdown.extension(), "md");

    assert_eq!(
        RenderedFormat::PlainText.artifact_schema(),
        "diagprint.report.text/v1",
    );
    assert_eq!(
        RenderedFormat::PlainText.media_type(),
        "text/plain; charset=utf-8",
    );
    assert_eq!(RenderedFormat::PlainText.extension(), "txt");
}

#[test]
fn markdown_and_plain_share_semantic_identity() {
    let report = sample_report();

    let markdown = MarkdownRenderer
        .render_report_artifact(&report)
        .expect("Markdown artifact should render");

    let plain = PlainRenderer
        .render_report_artifact(&report)
        .expect("plain artifact should render");

    assert_eq!(markdown.receipt().report, plain.receipt().report,);

    assert_ne!(
        markdown.receipt().artifact_digest,
        plain.receipt().artifact_digest,
    );

    assert_eq!(markdown.receipt().format, RenderedFormat::Markdown,);

    assert_eq!(plain.receipt().format, RenderedFormat::PlainText,);

    assert_eq!(markdown.receipt().schema, RENDERED_RECEIPT_V1_SCHEMA,);

    assert_eq!(plain.receipt().schema, RENDERED_RECEIPT_V1_SCHEMA,);
}

#[cfg(feature = "html")]
#[test]
fn html_markdown_and_plain_share_semantic_identity() {
    let report = sample_report();

    let html = HtmlRenderer::new()
        .render_report_artifact(&report)
        .expect("HTML artifact should render");

    let markdown = MarkdownRenderer
        .render_report_artifact(&report)
        .expect("Markdown artifact should render");

    let plain = PlainRenderer
        .render_report_artifact(&report)
        .expect("plain artifact should render");

    assert_eq!(html.receipt().report, markdown.receipt().report,);

    assert_eq!(html.receipt().report, plain.receipt().report,);

    assert_ne!(
        html.receipt().artifact_digest,
        markdown.receipt().artifact_digest,
    );

    assert_ne!(
        html.receipt().artifact_digest,
        plain.receipt().artifact_digest,
    );

    assert_ne!(
        markdown.receipt().artifact_digest,
        plain.receipt().artifact_digest,
    );
}

#[test]
fn rendered_artifact_verifies_against_its_receipt() {
    let report = sample_report();

    let artifact = MarkdownRenderer
        .render_report_artifact(&report)
        .expect("Markdown artifact should render");

    artifact.verify().expect("artifact should verify");

    assert_eq!(artifact.receipt().byte_length, artifact.bytes().len(),);

    assert_eq!(artifact.format(), RenderedFormat::Markdown,);

    assert!(
        artifact
            .as_str()
            .expect("Markdown should be UTF-8")
            .contains("parse failed")
    );
}

#[test]
fn tampered_rendered_bytes_fail_verification() {
    let report = sample_report();

    let artifact = PlainRenderer
        .render_report_artifact(&report)
        .expect("plain artifact should render");

    let mut tampered = artifact.bytes().to_vec();

    let index = tampered
        .iter()
        .position(|byte| byte.is_ascii_alphabetic())
        .expect("artifact should contain alphabetic data");

    tampered[index] = if tampered[index] == b'X' { b'Y' } else { b'X' };

    let error = artifact
        .receipt()
        .verify_bytes(&tampered)
        .expect_err("tampered bytes must fail");

    assert!(matches!(error, ArtifactVerificationError::Digest { .. }));
}

#[test]
fn markdown_source_artifact_records_cache_mode() {
    let reporter = reporter();
    let sources = SourceCache::new();

    let _ = sources.insert("src/main.rs", "fn main() {\n    let unused = 42;\n}\n");

    let report = DiagnosticReport::from_diagnostic(reporter.warning("unused binding").label(
        "src/main.rs",
        2,
        Some(9),
        Some(6),
        Some("unused"),
    ));

    let artifact = MarkdownRenderer
        .render_report_artifact_with_sources(&report, &sources)
        .expect("source-backed Markdown should render");

    assert_eq!(artifact.receipt().source.mode, RenderedSourceMode::Cache,);

    assert_eq!(artifact.receipt().source.context_lines, Some(2),);

    assert!(
        artifact
            .as_str()
            .expect("Markdown should be UTF-8")
            .contains("~~~rust")
    );
}

#[cfg(feature = "html")]
#[test]
fn html_source_artifact_records_cache_mode() {
    let reporter = reporter();
    let sources = SourceCache::new();

    let _ = sources.insert("src/main.rs", "fn main() {\n    let unused = 42;\n}\n");

    let report = DiagnosticReport::from_diagnostic(reporter.warning("unused binding").label(
        "src/main.rs",
        2,
        Some(9),
        Some(6),
        Some("unused"),
    ));

    let artifact = HtmlRenderer::new()
        .render_report_artifact_with_sources(&report, &sources)
        .expect("source-backed HTML should render");

    assert_eq!(artifact.receipt().source.mode, RenderedSourceMode::Cache,);

    assert_eq!(artifact.receipt().source.context_lines, Some(2),);

    assert!(
        artifact
            .as_str()
            .expect("HTML should be UTF-8")
            .contains("data-language=\"rust\"")
    );
}

#[test]
fn writer_persists_rendered_artifact_transactionally() {
    let report = sample_report();

    let artifact = MarkdownRenderer
        .render_report_artifact(&report)
        .expect("Markdown artifact should render");

    let destination = temporary_destination("rendered-artifact");

    let persisted = ArtifactWriter::new()
        .write_rendered(&artifact, &destination, "report.md")
        .expect("rendered artifact should persist");

    assert_eq!(
        fs::read(persisted.artifact_path()).expect("artifact should be readable"),
        artifact.bytes(),
    );

    assert!(persisted.receipt_path().ends_with("report.md.receipt.json"));

    let receipt = fs::read_to_string(persisted.receipt_path()).expect("receipt should be readable");

    assert!(receipt.contains(RENDERED_RECEIPT_V1_SCHEMA));
    assert!(receipt.contains("\"format\": \"markdown\""));

    assert_eq!(
        persisted.artifact_digest(),
        artifact.receipt().artifact_digest,
    );

    assert_eq!(persisted.byte_length(), artifact.receipt().byte_length,);

    fs::remove_dir_all(&destination).expect("test artifact directory should clean up");
}

#[test]
fn rendered_writer_preserves_create_only_behavior() {
    let report = sample_report();

    let artifact = PlainRenderer
        .render_report_artifact(&report)
        .expect("plain artifact should render");

    let destination = temporary_destination("rendered-create-only");

    let writer = ArtifactWriter::new();

    writer
        .write_rendered(&artifact, &destination, "report.txt")
        .expect("first write should succeed");

    let error = writer
        .write_rendered(&artifact, &destination, "report.txt")
        .expect_err("second write should fail");

    assert!(matches!(
        error,
        ArtifactWriteError::DestinationExists { .. }
    ));

    fs::remove_dir_all(&destination).expect("test artifact directory should clean up");
}
