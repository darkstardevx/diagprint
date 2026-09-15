use annotate_snippets::Renderer as SnippetRenderer;
use diagprint::{AnnotateSnippetsBridge, Reporter, Severity};

const SOURCE_NAME: &str = "memory://annotate-snippets/example.tao";

const SOURCE: &str = concat!("let left = 42;\n", "let right = \"forty-two\";\n",);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bridge = AnnotateSnippetsBridge::new(Severity::Error, "Cannot combine incompatible values")
        .source(SOURCE_NAME, SOURCE)
        .code("E-SNIPPET")
        .help("make both values use the same type")
        .primary_label(SOURCE_NAME, 4..8, "numeric value originates here")
        .secondary_label(SOURCE_NAME, 28..37, "string value originates here");

    println!("--- annotate-snippets ---");

    let groups = bridge.to_annotate_snippets_groups()?;

    println!(
        "{}",
        SnippetRenderer::plain().term_width(76).render(&groups)
    );

    println!();
    println!("--- diagprint ---");

    let reporter = Reporter::builder()
        .application("annotate-snippets-example")
        .color(false)
        .width(76)
        .sources_from(&bridge)
        .build()?;

    let diagnostic = bridge.to_diagprint(&reporter);

    reporter.emit(&diagnostic)?;

    Ok(())
}
