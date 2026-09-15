use diagprint::{AriadneBridge, Reporter, Severity};

const SOURCE_NAME: &str = "memory://ariadne/example.tao";

const SOURCE: &str = concat!("let left = 42;\n", "let right = \"forty-two\";\n",);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bridge = AriadneBridge::new(
        Severity::Error,
        "Cannot combine incompatible values",
        SOURCE_NAME,
        4..8,
    )
    .source(SOURCE_NAME, SOURCE)
    .code("E-ARIADNE")
    .help("make both values use the same type")
    .primary_label(SOURCE_NAME, 4..8, "numeric value originates here")
    .secondary_label(SOURCE_NAME, 28..37, "string value originates here");

    println!("--- Ariadne ---");

    let report = bridge.to_ariadne_report()?;

    report.print(ariadne::sources(bridge.ariadne_sources()))?;

    println!();
    println!("--- diagprint ---");

    let reporter = Reporter::builder()
        .application("ariadne-example")
        .color(false)
        .width(76)
        .sources_from(&bridge)
        .build()?;

    let diagnostic = bridge.to_diagprint(&reporter);

    reporter.emit(&diagnostic)?;

    Ok(())
}
