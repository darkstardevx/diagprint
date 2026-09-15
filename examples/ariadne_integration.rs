use diagprint::{
    AriadneBridge, Reporter, Severity,
    render::{Renderer, TerminalRenderer},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const SOURCE: &str = include_str!("fixtures/ariadne_sample.tao");

    let source_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/fixtures/ariadne_sample.tao"
    );

    let bridge = AriadneBridge::new(
        Severity::Error,
        "Cannot combine incompatible values",
        source_path,
        4..8,
    )
    .source(source_path, SOURCE)
    .code("E-ARIADNE")
    .help("make both values use the same type")
    .primary_label(source_path, 4..8, "numeric value originates here")
    .secondary_label(source_path, 28..37, "string value originates here");

    let ariadne_report = bridge.to_ariadne_report()?;

    ariadne_report.print(ariadne::sources(bridge.ariadne_sources()))?;

    let reporter = Reporter::builder()
        .application("ariadne-example")
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
