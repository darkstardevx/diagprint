use diagprint::{
    ArtifactWriter, DiagnosticReport, Reporter, SourceCache,
    render::{MarkdownRenderer, PlainRenderer},
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

    let markdown_destination = format!("target/diagprint-markdown-{nonce}");

    let markdown_persisted =
        writer.write_rendered(&markdown, &markdown_destination, "report.md")?;

    println!("Markdown: {}", markdown_persisted.artifact_path().display());

    println!("           {}", markdown_persisted.receipt_path().display());

    let plain = PlainRenderer.render_report_artifact(&report)?;

    let plain_destination = format!("target/diagprint-plain-{nonce}");

    let plain_persisted = writer.write_rendered(&plain, &plain_destination, "report.txt")?;

    println!("Plain:    {}", plain_persisted.artifact_path().display());

    println!("           {}", plain_persisted.receipt_path().display());

    #[cfg(feature = "html")]
    {
        let html = HtmlRenderer::new()
            .with_theme(HtmlTheme::Auto)
            .with_title("diagprint rendered artifact example")
            .render_report_artifact_with_sources(&report, &sources)?;

        let html_destination = format!("target/diagprint-html-{nonce}");

        let html_persisted = writer.write_rendered(&html, &html_destination, "report.html")?;

        println!("HTML:     {}", html_persisted.artifact_path().display());

        println!("           {}", html_persisted.receipt_path().display());

        println!();
        println!("Semantic report digest: {}", html.receipt().report);
    }

    println!();
    println!(
        "Markdown artifact digest: {}",
        markdown.receipt().artifact_digest
    );

    println!(
        "Plain artifact digest:    {}",
        plain.receipt().artifact_digest
    );

    Ok(())
}
