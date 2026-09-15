use anyhow::{Context, Result};
use diagprint::{AnyhowDiagnosticExt, Reporter};
use std::io;

fn simulated_config_load() -> Result<()> {
    let error = io::Error::new(
        io::ErrorKind::NotFound,
        "/etc/cyberdeck/config.toml does not exist",
    );

    Err::<(), _>(error)
        .context("failed to read application configuration")
        .context("application startup failed")
}

fn main() -> diagprint::Result<()> {
    let reporter = Reporter::builder()
        .application("diagprint-anyhow")
        .width(88)
        .build()?;

    let error = simulated_config_load().expect_err("the example intentionally creates an error");

    let diagnostic = error
        .to_diagprint(&reporter)
        .code("CONFIG-001")
        .note("The original anyhow context chain was preserved.");

    reporter.emit(&diagnostic)?;

    Ok(())
}
