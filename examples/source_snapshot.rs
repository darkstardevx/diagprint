use diagprint::{Reporter, render::TerminalRenderer};

const SOURCE_NAME: &str = "memory://editor/demo.rs";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Reporter::builder()
        .application("source-snapshot-example")
        .source(SOURCE_NAME, "let answer = old_value();\n")
        .build()?;

    let diagnostic = reporter
        .error("Diagnostic from an earlier editor revision")
        .code("E-SNAPSHOT")
        .label(
            SOURCE_NAME,
            1,
            Some(13),
            Some(9),
            Some("this diagnostic points at the old buffer"),
        );

    let snapshot = reporter.source_snapshot();

    reporter.register_source(SOURCE_NAME, "let answer = new_value();\n");

    let renderer = TerminalRenderer {
        color: false,
        width: 76,
        ..Default::default()
    };

    println!("--- immutable diagnostic snapshot ---");

    print!("{}", renderer.render_with_snapshot(&diagnostic, &snapshot,));

    println!();
    println!("--- current live editor buffer ---");

    let cache = reporter.source_cache();

    print!("{}", renderer.render_with_sources(&diagnostic, &cache,));

    Ok(())
}
