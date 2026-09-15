use diagprint::render::{JsonRenderer, SarifRenderer};
use diagprint::{DiagnosticReport, ReportRenderer, ReportStatus, Reporter, Severity};

fn report() -> DiagnosticReport {
    let reporter = Reporter::builder()
        .application("batch-test")
        .build()
        .unwrap();

    [
        reporter.info("information").code("INFO-1"),
        reporter.warning("warning").code("WARN-1"),
        reporter.error("failure").code("ERR-1"),
    ]
    .into_iter()
    .collect()
}

#[test]
fn report_status_uses_error_threshold_by_default() {
    let report = report();

    assert_eq!(report.status(), ReportStatus::Failure);

    assert_eq!(report.exit_code(), 1);

    assert_eq!(report.count_at_or_above(Severity::Warning,), 2);
}

#[test]
fn report_status_supports_custom_thresholds() {
    let reporter = Reporter::builder()
        .application("threshold-test")
        .build()
        .unwrap();

    let report: DiagnosticReport = [reporter.info("info"), reporter.warning("warning")]
        .into_iter()
        .collect();

    assert_eq!(report.status(), ReportStatus::Success);

    assert_eq!(report.exit_code(), 0);

    assert_eq!(report.status_at(Severity::Warning,), ReportStatus::Failure);

    assert_eq!(report.exit_code_at(Severity::Warning,), 1);
}

#[test]
fn json_report_is_one_valid_array() {
    let report = report();

    let rendered = JsonRenderer.render_report(report.iter());

    let json: serde_json::Value = serde_json::from_str(&rendered).unwrap();

    assert_eq!(json.as_array().unwrap().len(), 3);
}

#[test]
fn sarif_report_contains_all_results() {
    let report = report();

    let rendered = SarifRenderer.render_report(report.iter());

    let json: serde_json::Value = serde_json::from_str(&rendered).unwrap();

    assert_eq!(json["runs"][0]["results"].as_array().unwrap().len(), 3);
}

#[test]
fn filtered_report_preserves_original() {
    let report = report();

    let filtered = report.filtered_min_severity(Severity::Warning);

    assert_eq!(report.len(), 3);
    assert_eq!(filtered.len(), 2);
}
