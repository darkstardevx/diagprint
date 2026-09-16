use diagprint::{
    ArtifactWriter, DiagnosticReport, Reporter, SourceCache,
    render::{AuditTranscriptRenderer, CompilerTextRenderer, MarkdownRenderer, PlainRenderer},
};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "html")]
use diagprint::render::{HtmlRenderer, HtmlTheme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Reporter::builder()
        .application("diagprint-rendered-example")
        .color(false)
        .build()?;

    let sources = SourceCache::new();

    let _ = sources.insert(
        "src/main.rs",
        concat!(
            "fn main() {\n",
            "    let answer = \"forty-two\";\n",
            "    println!(\"answer = {answer}\");\n",
            "}\n",
        ),
    );

    let warning = reporter
        .warning("value remains textual")
        .code("example::string-value")
        .label(
            "src/main.rs",
            2,
            Some(9),
            Some(6),
            Some("binding declared here"),
        )
        .note("Rendered artifacts preserve one semantic report identity.")
        .help("Choose the representation appropriate for humans or automation.");

    let error = reporter
        .error("expected integer, found string")
        .code("E0308")
        .label("src/main.rs", 2, Some(18), Some(11), Some("string literal"));

    let mut report = DiagnosticReport::new();
    report.push(warning).push(error);

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();

    let writer = ArtifactWriter::new();

    let markdown = MarkdownRenderer.render_report_artifact_with_sources(&report, &sources)?;

    let markdown_persisted = writer.write_rendered(
        &markdown,
        format!("target/diagprint-markdown-{nonce}"),
        "report.md",
    )?;

    println!("Markdown: {}", markdown_persisted.artifact_path().display());

    let plain = PlainRenderer.render_report_artifact(&report)?;

    let plain_persisted = writer.write_rendered(
        &plain,
        format!("target/diagprint-plain-{nonce}"),
        "report.txt",
    )?;

    println!("Plain:    {}", plain_persisted.artifact_path().display());

    let compiler = CompilerTextRenderer.render_report_artifact(&report)?;

    let compiler_persisted = writer.write_rendered(
        &compiler,
        format!("target/diagprint-compiler-{nonce}"),
        "report.txt",
    )?;

    println!("Compiler: {}", compiler_persisted.artifact_path().display());

    let audit = AuditTranscriptRenderer.render_report_artifact(&report)?;

    let audit_persisted = writer.write_rendered(
        &audit,
        format!("target/diagprint-audit-{nonce}"),
        "report.audit",
    )?;

    println!("Audit:    {}", audit_persisted.artifact_path().display());

    #[cfg(feature = "html")]
    {
        let html = HtmlRenderer::new()
            .with_theme(HtmlTheme::Auto)
            .with_title("diagprint rendered artifact example")
            .render_report_artifact_with_sources(&report, &sources)?;

        let html_persisted = writer.write_rendered(
            &html,
            format!("target/diagprint-html-{nonce}"),
            "report.html",
        )?;

        println!("HTML:     {}", html_persisted.artifact_path().display());
    }

    println!();
    println!("Semantic report digest: {}", audit.receipt().report);

    println!(
        "Markdown artifact:      {}",
        markdown.receipt().artifact_digest
    );

    println!(
        "Plain artifact:         {}",
        plain.receipt().artifact_digest
    );

    println!(
        "Compiler artifact:      {}",
        compiler.receipt().artifact_digest
    );

    println!(
        "Audit artifact:         {}",
        audit.receipt().artifact_digest
    );

    Ok(())
}
