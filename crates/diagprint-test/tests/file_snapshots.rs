use diagprint::{DiagnosticReport, Reporter};
use diagprint_test::{
    assert_diagnostic_file_snapshot, assert_diagnostic_snapshot, assert_report_file_snapshot,
    assert_report_snapshot,
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

#[test]
fn inline_diagnostic_snapshot_matches_semantic_content() {
    let diagnostic = reporter().warning("unused binding").code("lint::unused");

    assert_diagnostic_snapshot!(
        diagnostic,
        r#"
        {
          "application": "diagprint-test",
          "severity": "warning",
          "code": "lint::unused",
          "message": "unused binding",
          "labels": [],
          "notes": [],
          "help": null,
          "cause": null,
          "suggestions": []
        }
        "#,
    );
}

#[test]
fn inline_report_snapshot_matches_semantic_content() {
    let reporter = reporter();

    let mut report = DiagnosticReport::new();

    report.push(reporter.error("parse failed").code("parse::failed"));

    assert_report_snapshot!(
        report,
        r#"
        [
          {
            "application": "diagprint-test",
            "severity": "error",
            "code": "parse::failed",
            "message": "parse failed",
            "labels": [],
            "notes": [],
            "help": null,
            "cause": null,
            "suggestions": []
          }
        ]
        "#,
    );
}

#[test]
fn diagnostic_file_snapshot_matches_committed_json() {
    let diagnostic = reporter().warning("unused binding").code("lint::unused");

    assert_diagnostic_file_snapshot!(diagnostic, "tests/snapshots/diagnostic-warning.json",);
}

#[test]
fn report_file_snapshot_matches_committed_json() {
    let reporter = reporter();

    let mut report = DiagnosticReport::new();

    report.push(reporter.error("parse failed").code("parse::failed"));

    assert_report_file_snapshot!(report, "tests/snapshots/report-error.json",);
}

#[test]
fn report_snapshot_is_independent_of_insertion_order() {
    let reporter = reporter();

    let info = reporter
        .info("analysis complete")
        .code("analysis::complete");

    let error = reporter.error("parse failed").code("parse::failed");

    let mut first = DiagnosticReport::new();
    first.push(info.clone()).push(error.clone());

    let mut second = DiagnosticReport::new();
    second.push(error).push(info);

    assert_eq!(
        diagprint_test::report_snapshot(&first),
        diagprint_test::report_snapshot(&second),
    );
}
