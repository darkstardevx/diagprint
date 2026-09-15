use diagprint::{Reporter, SourceCache, render::TerminalRenderer};

const NAME: &str = "memory://editor/revision-bound-demo.rs";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = SourceCache::new();

    cache.insert(NAME, "let answer = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("revision-bound-demo")
        .build()?;

    let diagnostic = reporter
        .error("Diagnostic belongs to an older editor revision")
        .code("E-REVISION")
        .label(
            NAME,
            1,
            Some(14),
            Some(9),
            Some("value when the diagnostic was created"),
        )
        .bind_source_revisions(&snapshot);

    cache.insert(NAME, "let answer = new_value();\n");

    let renderer = TerminalRenderer {
        color: false,
        width: 76,
        ..Default::default()
    };

    println!("--- live cache: diagnostic is stale ---");

    print!("{}", renderer.render_with_sources(&diagnostic, &cache,));

    println!();
    println!("--- diagnostic snapshot: exact source ---");

    print!("{}", renderer.render_with_snapshot(&diagnostic, &snapshot,));

    Ok(())
}
