use diagprint::{
    DIAGNOSTIC_HISTORY_RUN_V2_SCHEMA, DiagnosticHistory, DiagnosticHistoryError, DiagnosticReport,
    Reporter,
};
use serde_json::Value;
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

fn sample_report(severity: &str) -> DiagnosticReport {
    let reporter = reporter();

    match severity {
        "warning" => {
            DiagnosticReport::from_diagnostic(reporter.warning("configuration drift").code("W100"))
        }

        "error" => {
            DiagnosticReport::from_diagnostic(reporter.error("configuration drift").code("W100"))
        }

        other => {
            panic!("unsupported fixture severity {other}");
        }
    }
}

#[test]
fn history_is_hash_chained_and_reopens() {
    let root = temporary_directory("chain");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("scan-1", &sample_report("warning"))
        .expect("first run should append");

    history
        .append_report("scan-2", &sample_report("error"))
        .expect("second run should append");

    assert!(root.join("head.json",).is_file());

    let first = &history.runs()[0];

    let second = &history.runs()[1];

    assert_eq!(first.schema, DIAGNOSTIC_HISTORY_RUN_V2_SCHEMA,);

    assert_eq!(first.previous_run_digest, None,);

    assert_eq!(second.previous_run_digest, Some(first.run_digest),);

    assert_eq!(history.head_digest(), Some(second.run_digest),);

    history.verify().expect("history should verify");

    drop(history);

    let reopened = DiagnosticHistory::open(&root).expect("history should reopen");

    assert_eq!(reopened.len(), 2,);

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn edited_historical_run_is_detected() {
    let root = temporary_directory("tamper-run");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("baseline", &sample_report("warning"))
        .expect("run should append");

    drop(history);

    let path = root.join("run-000000.json");

    let mut value: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("run should be readable"))
            .expect("run should parse");

    value["label"] = Value::String("tampered".to_owned());

    fs::write(
        &path,
        serde_json::to_vec_pretty(&value).expect("tampered JSON should serialize"),
    )
    .expect("run should be rewritten");

    let error = DiagnosticHistory::open(&root).expect_err("tampering must be detected");

    assert!(matches!(
        error,
        DiagnosticHistoryError::RunDigestMismatch { .. }
    ));

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn reordered_history_is_detected() {
    let root = temporary_directory("reorder");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("scan-1", &sample_report("warning"))
        .expect("run should append");

    history
        .append_report("scan-2", &sample_report("error"))
        .expect("run should append");

    drop(history);

    let first = root.join("run-000000.json");

    let second = root.join("run-000001.json");

    let first_bytes = fs::read(&first).expect("first run should read");

    let second_bytes = fs::read(&second).expect("second run should read");

    fs::write(&first, second_bytes).expect("first run should swap");

    fs::write(&second, first_bytes).expect("second run should swap");

    let error = DiagnosticHistory::open(&root).expect_err("reordering must be detected");

    assert!(matches!(
        error,
        DiagnosticHistoryError::RunIndexMismatch { .. }
            | DiagnosticHistoryError::PreviousDigestMismatch { .. }
            | DiagnosticHistoryError::RunDigestMismatch { .. }
    ));

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn truncated_tail_is_detected_by_head_record() {
    let root = temporary_directory("truncate");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("scan-1", &sample_report("warning"))
        .expect("run should append");

    history
        .append_report("scan-2", &sample_report("error"))
        .expect("run should append");

    drop(history);

    fs::remove_file(root.join("run-000001.json")).expect("tail run should be deleted");

    let error = DiagnosticHistory::open(&root).expect_err("truncation must be detected");

    assert!(matches!(
        error,
        DiagnosticHistoryError::HeadRunCountMismatch {
            expected: 2,
            actual: 1,
        }
    ));

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn history_detects_changed_severity() {
    let root = temporary_directory("changed");

    let first = sample_report("warning");

    let second = sample_report("error");

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

    assert_eq!(transition.severity_increases, 1,);

    assert_eq!(transition.introduced_errors, 1,);

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn lineage_detects_resolution_and_reappearance() {
    let root = temporary_directory("lineage");

    let finding = reporter().warning("persistent finding").code("W300");

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

    let report = DiagnosticReport::from_diagnostic(
        reporter()
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

    assert!(persisted.contains("run_digest",));

    assert!(!persisted.contains("super-secret diagnostic message",));

    assert!(!persisted.contains("private remediation guidance",));

    fs::remove_dir_all(root).expect("history should clean up");
}
