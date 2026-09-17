use diagprint::Reporter;
use diagprint_error_stack::ErrorStackReportExt;
use error_stack::Report;
use std::{error::Error, fmt};

#[derive(Debug)]
struct StorageError;

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("storage read failed")
    }
}

impl Error for StorageError {}

#[derive(Debug)]
struct ApplicationError;

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("application startup failed")
    }
}

impl Error for ApplicationError {}

fn main() -> Result<(), Box<dyn Error>> {
    let reporter = Reporter::builder()
        .application("error-stack-bridge-example")
        .build()?;

    let report = Report::new(StorageError).change_context(ApplicationError);
    let converted = report.to_diagprint(&reporter)?;

    reporter.emit_report(converted.report())?;

    println!(
        "relationship graph: nodes={} edges={}",
        converted.graph().node_count(),
        converted.graph().edge_count(),
    );

    Ok(())
}
