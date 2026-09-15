use diagprint::{Reporter, render::TerminalRenderer};

const NAME: &str = "memory://editor/captured-demo.rs";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Reporter::builder()
        .application("captured-diagnostic-demo")
        .source(NAME, "let answer = old_value();\n")
        .build()?;

    let captured = reporter.capture(
        reporter
            .error("Diagnostic captured from editor revision")
            .code("E-CAPTURED")
            .label(
                NAME,
                1,
                Some(14),
                Some(9),
                Some("source at diagnostic time"),
            ),
    );

    reporter.register_source(NAME, "let answer = new_value();\n");

    println!(
        "stale against live buffer: {}",
        captured.is_stale(&reporter.source_cache())
    );

    println!();

    let renderer = TerminalRenderer {
        color: false,
        width: 76,
        ..Default::default()
    };

    print!("{}", renderer.render_captured(&captured));

    Ok(())
}
