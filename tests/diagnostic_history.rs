use diagprint::{DiagnosticHistory, DiagnosticReport, Reporter};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagnostic-history-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-history-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

#[test]
fn history_is_append_only_and_reopens() {
    let root = temporary_directory("append");

    let reporter = reporter();

    let first =
        DiagnosticReport::from_diagnostic(reporter.warning("configuration drift").code("W100"));

    let second =
        DiagnosticReport::from_diagnostic(reporter.error("configuration drift").code("W100"));

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("scan-1", &first)
        .expect("first run should append");

    history
        .append_report("scan-2", &second)
        .expect("second run should append");

    assert!(root.join("run-000000.json",).is_file());

    assert!(root.join("run-000001.json",).is_file());

    drop(history);

    let reopened = DiagnosticHistory::open(&root).expect("history should reopen");

    assert_eq!(reopened.len(), 2,);

    assert_eq!(reopened.runs()[0].label, "scan-1",);

    assert_eq!(reopened.runs()[1].label, "scan-2",);

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn history_detects_changed_severity() {
    let root = temporary_directory("changed");

    let reporter = reporter();

    let first =
        DiagnosticReport::from_diagnostic(reporter.warning("configuration drift").code("W200"));

    let second =
        DiagnosticReport::from_diagnostic(reporter.error("configuration drift").code("W200"));

    assert_eq!(
        first
            .iter()
            .next()
            .expect("diagnostic should exist",)
            .fingerprint(),
        second
            .iter()
            .next()
            .expect("diagnostic should exist",)
            .fingerprint(),
    );

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("before", &first)
        .expect("run should append");

    history
        .append_report("after", &second)
        .expect("run should append");

    let transition = history
        .latest_transition()
        .expect("transition should build")
        .expect("transition should exist");

    assert_eq!(transition.counts.changed, 1,);

    assert_eq!(transition.counts.new, 0,);

    assert_eq!(transition.counts.resolved, 0,);

    assert_eq!(transition.severity_increases, 1,);

    assert_eq!(transition.introduced_errors, 1,);

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn lineage_detects_resolution_and_reappearance() {
    let root = temporary_directory("lineage");

    let reporter = reporter();

    let finding = reporter.warning("persistent finding").code("W300");

    let fingerprint = finding.fingerprint().qualified();

    let present = DiagnosticReport::from_diagnostic(finding);

    let absent = DiagnosticReport::new();

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("scan-1", &present)
        .expect("run should append");

    history
        .append_report("scan-2", &absent)
        .expect("run should append");

    history
        .append_report("scan-3", &present)
        .expect("run should append");

    let lineage = history.lineage(&fingerprint);

    assert_eq!(lineage.first_seen_run(), Some(0),);

    assert_eq!(lineage.last_seen_run(), Some(2),);

    assert!(lineage.active_in_latest());

    assert!(lineage.reappeared_after_absence());

    assert_eq!(lineage.resolutions(), 1,);

    assert_eq!(lineage.appearances(), 2,);

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn persisted_history_does_not_store_diagnostic_text() {
    let root = temporary_directory("privacy");

    let reporter = reporter();

    let report = DiagnosticReport::from_diagnostic(
        reporter
            .error("super-secret diagnostic message")
            .code("E900")
            .help("private remediation guidance"),
    );

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("private-scan", &report)
        .expect("run should append");

    let persisted =
        fs::read_to_string(root.join("run-000000.json")).expect("history run should be readable");

    assert!(persisted.contains("fingerprint",));

    assert!(persisted.contains("digest",));

    assert!(!persisted.contains("super-secret diagnostic message",));

    assert!(!persisted.contains("private remediation guidance",));

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn identical_diagnostics_persist_across_runs() {
    let root = temporary_directory("persist");

    let reporter = reporter();

    let report = DiagnosticReport::from_diagnostic(reporter.warning("stable finding").code("W500"));

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("scan-1", &report)
        .expect("run should append");

    history
        .append_report("scan-2", &report)
        .expect("run should append");

    let transition = history
        .latest_transition()
        .expect("transition should build")
        .expect("transition should exist");

    assert_eq!(transition.counts.persisting, 1,);

    assert_eq!(transition.counts.differences(), 0,);

    fs::remove_dir_all(root).expect("history should clean up");
}
