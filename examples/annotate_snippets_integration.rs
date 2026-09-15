use annotate_snippets::Renderer as SnippetRenderer;
use diagprint::{
    AnnotateSnippetsBridge, Reporter, Severity,
    render::{Renderer, TerminalRenderer},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const SOURCE: &str = include_str!("fixtures/ariadne_sample.tao");

    let source_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/fixtures/ariadne_sample.tao"
    );

    let bridge = AnnotateSnippetsBridge::new(Severity::Error, "Cannot combine incompatible values")
        .source(source_path, SOURCE)
        .code("E-SNIPPET")
        .help("make both values use the same type")
        .primary_label(source_path, 4..8, "numeric value originates here")
        .secondary_label(source_path, 28..37, "string value originates here");

    let groups = bridge.to_annotate_snippets_groups()?;

    println!(
        "{}",
        SnippetRenderer::plain().term_width(76).render(&groups)
    );

    let reporter = Reporter::builder()
        .application("annotate-snippets-example")
        .build()
        .unwrap();

    let diagnostic = bridge.to_diagprint(&reporter);

    let renderer = TerminalRenderer {
        color: false,
        width: 76,
        ..Default::default()
    };

    println!();
    print!("{}", renderer.render(&diagnostic));

    Ok(())
}
