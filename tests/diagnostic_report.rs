use diagprint::{DiagnosticReport, Reporter, Severity};

#[test]
fn report_tracks_counts_and_highest_severity() {
    let reporter = Reporter::builder()
        .application("report-test")
        .build()
        .unwrap();

    let report: DiagnosticReport = [
        reporter.info("info"),
        reporter.warning("warning"),
        reporter.error("error"),
        reporter.fatal("fatal"),
    ]
    .into_iter()
    .collect();

    let counts = report.counts();

    assert_eq!(counts.info, 1);
    assert_eq!(counts.warning, 1);
    assert_eq!(counts.error, 1);
    assert_eq!(counts.fatal, 1);
    assert_eq!(counts.total(), 4);
    assert_eq!(counts.failures(), 2);

    assert_eq!(report.highest_severity(), Some(Severity::Fatal));

    assert!(report.has_errors());
    assert!(report.has_warnings());
}

#[test]
fn report_can_filter_by_minimum_severity() {
    let reporter = Reporter::builder()
        .application("report-test")
        .build()
        .unwrap();

    let mut report: DiagnosticReport = [
        reporter.debug("debug"),
        reporter.warning("warning"),
        reporter.error("error"),
    ]
    .into_iter()
    .collect();

    report.retain_min_severity(Severity::Warning);

    assert_eq!(report.len(), 2);
}

#[test]
fn deterministic_sort_orders_failures_first() {
    let reporter = Reporter::builder()
        .application("report-test")
        .build()
        .unwrap();

    let mut report: DiagnosticReport = [
        reporter.warning("z warning"),
        reporter.error("b error").code("E2"),
        reporter.error("a error").code("E1"),
    ]
    .into_iter()
    .collect();

    report.sort_deterministic();

    let messages: Vec<_> = report
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();

    assert_eq!(messages, ["a error", "b error", "z warning"]);
}
