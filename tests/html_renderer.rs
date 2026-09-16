#![cfg(feature = "html")]

use diagprint::{
    DiagnosticReport, Reporter, SourceCache,
    render::{HtmlRenderer, HtmlSourceOptions, HtmlTheme, Renderer},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("html-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

#[test]
fn html_renderer_produces_a_complete_standalone_document() {
    let diagnostic = reporter()
        .error("parse failed")
        .code("parse::failed")
        .help("check the expression");

    let output = HtmlRenderer::new().render(&diagnostic);

    assert!(output.starts_with("<!doctype html>"));
    assert!(output.contains("<html"));
    assert!(output.contains("<style>"));
    assert!(output.contains("severity-error"));
    assert!(output.contains("parse::failed"));
    assert!(output.contains("parse failed"));
    assert!(output.contains("Diagnostic metadata"));
    assert!(output.ends_with("</html>\n"));
}

#[test]
fn html_renderer_escapes_diagnostic_content() {
    let diagnostic = reporter()
        .error("<script>alert(\"boom\")</script>")
        .code("<unsafe>");

    let output = HtmlRenderer::new().render(&diagnostic);

    assert!(!output.contains("<script>alert"));
    assert!(output.contains("&lt;script&gt;alert"));
    assert!(output.contains("&lt;unsafe&gt;"));
}

#[test]
fn html_source_rendering_includes_highlighted_rust() {
    let sources = SourceCache::new();

    let _ = sources.insert(
        "src/main.rs",
        "fn main() {\n    let unused = 42;\n    println!(\"done\");\n}\n",
    );

    let diagnostic = reporter()
        .warning("unused binding")
        .code("lint::unused")
        .label("src/main.rs", 2, Some(9), Some(6), Some("unused binding"));

    let output = HtmlRenderer::new().render_with_sources(&diagnostic, &sources);

    assert!(output.contains("data-language=\"rust\""));
    assert!(output.contains("source-target-band"));
    assert!(output.contains("target-line-number"));
    assert!(output.contains("^^^^^^"));
    assert!(output.contains("unused binding"));

    // Syntect emits prefixed semantic classes.
    assert!(output.contains("dp-source"));
}

#[test]
fn html_source_context_is_configurable() {
    let sources = SourceCache::new();

    let _ = sources.insert(
        "src/lib.rs",
        "const ONE: u8 = 1;\n\
         const TWO: u8 = 2;\n\
         const THREE: u8 = 3;\n\
         const FOUR: u8 = 4;\n\
         const FIVE: u8 = 5;\n",
    );

    let diagnostic =
        reporter()
            .error("bad constant")
            .label("src/lib.rs", 3, Some(7), Some(5), Some("target"));

    let options = HtmlSourceOptions::new().with_context_lines(1);

    let output =
        HtmlRenderer::new().render_with_sources_and_options(&diagnostic, &sources, options);

    // Syntect inserts HTML spans between syntax tokens, so assertions should
    // target identifiers rather than complete unhighlighted source lines.
    assert!(!output.contains(">ONE<"));
    assert!(output.contains(">TWO<"));
    assert!(output.contains(">THREE<"));
    assert!(output.contains(">FOUR<"));
    assert!(!output.contains(">FIVE<"));

    assert!(output.contains("lines <code>2–4</code>"));
}

#[test]
fn stale_source_revision_is_not_embedded() {
    let sources = SourceCache::new();

    let revision = sources.insert_revisioned("src/main.rs", "fn old_version() {}\n");

    let diagnostic = reporter().error("stale diagnostic").label_at_revision(
        "src/main.rs",
        revision,
        1,
        Some(4),
        Some(11),
        Some("old source"),
    );

    let _ = sources.insert("src/main.rs", "fn new_version() {}\n");

    let output = HtmlRenderer::new().render_with_sources(&diagnostic, &sources);

    assert!(output.contains("Stale source revision"));
    assert!(!output.contains("fn old_version"));
    assert!(!output.contains("fn new_version"));
}

#[test]
fn explicit_light_and_dark_themes_are_emitted() {
    let diagnostic = reporter().info("theme test");

    let light = HtmlRenderer::new()
        .with_theme(HtmlTheme::Light)
        .render(&diagnostic);

    let dark = HtmlRenderer::new()
        .with_theme(HtmlTheme::Dark)
        .render(&diagnostic);

    assert!(light.contains("data-theme=\"light\""));
    assert!(dark.contains("data-theme=\"dark\""));
}

#[test]
fn report_renderer_produces_one_html_document() {
    let reporter = reporter();

    let mut report = DiagnosticReport::new();

    report
        .push(reporter.warning("warning one").code("warning::one"))
        .push(reporter.error("error two").code("error::two"));

    let output = HtmlRenderer::new()
        .with_title("Build diagnostics")
        .render_report(report.iter());

    assert_eq!(output.matches("<!doctype html>").count(), 1);
    assert_eq!(output.matches("<article class=\"diagnostic ").count(), 2);

    assert!(output.contains("Build diagnostics"));
    assert!(output.contains("warning one"));
    assert!(output.contains("error two"));
    assert!(output.contains("2 diagnostics"));
}

#[test]
fn metadata_can_be_disabled() {
    let diagnostic = reporter().info("quiet metadata");

    let output = HtmlRenderer::new().with_metadata(false).render(&diagnostic);

    assert!(!output.contains("Diagnostic metadata"));
}

#[test]
fn javascript_documentation_urls_are_not_clickable() {
    use diagprint::Suggestion;

    let suggestion = Suggestion::new("Read docs").documentation(diagprint::DocumentationLink::new(
        "unsafe link",
        "javascript:alert(1)",
    ));

    let diagnostic = reporter()
        .warning("unsafe documentation URL")
        .suggestion(suggestion);

    let output = HtmlRenderer::new().render(&diagnostic);

    assert!(output.contains("unsafe link"));
    assert!(!output.contains("href=\"javascript:"));
}
