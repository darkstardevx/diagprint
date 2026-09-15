use diagprint::{Reporter, TracingLayer};
use std::{error::Error, io};
use tracing_subscriber::prelude::*;

fn main() -> diagprint::Result<()> {
    let reporter = Reporter::builder()
        .application("diagprint-tracing")
        .width(88)
        .build()?;

    let layer = TracingLayer::new(reporter);

    let failure_monitor = layer.clone();

    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        let startup = tracing::info_span!("startup", component = "configuration");

        let _guard = startup.enter();

        let error = io::Error::new(
            io::ErrorKind::NotFound,
            "/etc/cyberdeck/config.toml does not exist",
        );

        tracing::error!(
            diag.code = "CONFIG-TRACE-001",
            diag.help = "Create the configuration file or update its path.",
            diag.note = "Captured directly from a tracing event.",
            path = "/etc/cyberdeck/config.toml",
            error = &error as &(dyn Error + 'static),
            "configuration loading failed"
        );

        tracing::warn!(
            diag.code = "CONFIG-TRACE-002",
            attempts = 3_u64,
            "configuration fallback exhausted"
        );

        // INFO is below TracingLayer's default WARNING threshold and is
        // intentionally ignored.
        tracing::info!("this informational event does not become a rich diagnostic");
    });

    let failures = failure_monitor.take_emission_failures();

    if !failures.is_empty() {
        eprintln!("diagprint tracing emission failures:");

        for failure in failures {
            eprintln!("  {failure}");
        }
    }

    Ok(())
}
