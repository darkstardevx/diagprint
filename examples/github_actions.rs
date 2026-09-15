use diagprint::Reporter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Reporter::builder()
        .application("github-actions-example")
        .build()?;

    let diagnostic = reporter
        .error("Cannot combine incompatible values")
        .code("E-CI")
        .label("src/main.rs", 12, Some(9), Some(5), Some("numeric value"))
        .secondary_label(
            "src/lib.rs",
            4,
            Some(5),
            Some(8),
            Some("string declaration"),
        )
        .help("make both values use the same type");

    reporter.emit_github_actions(&diagnostic)?;

    Ok(())
}
