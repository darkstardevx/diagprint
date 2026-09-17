use diagprint::{
    DiagnosticHistory, DiagnosticReport, GitProvenanceBinding, GitProvenanceError,
    GitProvenanceRecord, Reporter,
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-git-provenance-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn report() -> DiagnosticReport {
    let reporter = Reporter::builder()
        .application("git-provenance-test")
        .color(false)
        .build()
        .expect("reporter should build");

    let mut report = DiagnosticReport::new();

    report.push(
        reporter
            .warning("private diagnostic message")
            .code("git::provenance::test"),
    );

    report
}

#[test]
fn captured_record_round_trips_and_binds_exact_run() {
    let root = temporary_directory("roundtrip");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("baseline", &report())
        .expect("history should append");

    let run = history.latest().expect("history should contain a run");

    let record = GitProvenanceRecord::captured_clean(
        run,
        "a".repeat(40),
        "b".repeat(40),
        vec!["c".repeat(40)],
    )
    .expect("record should build");

    assert_eq!(record.binding, GitProvenanceBinding::CapturedClean,);

    let path = record.persist(&history).expect("record should persist");

    assert!(path.is_file(),);

    let loaded = GitProvenanceRecord::load(&history, 0)
        .expect("record should load")
        .expect("record should exist");

    assert_eq!(loaded, record,);

    loaded
        .verify_against(run)
        .expect("record should verify against exact run");

    let persisted = fs::read_to_string(path).expect("record should be readable");

    assert!(
        !persisted.contains("private diagnostic message",),
        "Git provenance must not regain diagnostic message text",
    );

    fs::remove_dir_all(root).expect("test should clean up");
}

#[test]
fn exact_duplicate_persist_is_idempotent() {
    let root = temporary_directory("idempotent");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("baseline", &report())
        .expect("history should append");

    let run = history.latest().expect("history should contain run");

    let record =
        GitProvenanceRecord::captured_clean(run, "1".repeat(40), "2".repeat(40), Vec::new())
            .expect("record should build");

    let first = record
        .persist(&history)
        .expect("first persist should succeed");

    let second = record
        .persist(&history)
        .expect("identical second persist should be idempotent");

    assert_eq!(first, second,);

    fs::remove_dir_all(root).expect("test should clean up");
}

#[test]
fn tampered_record_digest_is_rejected() {
    let root = temporary_directory("tamper");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("baseline", &report())
        .expect("history should append");

    let run = history.latest().expect("history should contain run");

    let record =
        GitProvenanceRecord::user_asserted(run, "3".repeat(40), "4".repeat(40), Vec::new())
            .expect("record should build");

    let path = record.persist(&history).expect("record should persist");

    let mut value: Value = serde_json::from_slice(&fs::read(&path).expect("record should read"))
        .expect("record JSON should parse");

    value["commit"] = Value::String("5".repeat(40));

    fs::write(
        &path,
        serde_json::to_vec_pretty(&value).expect("tampered JSON should encode"),
    )
    .expect("tampered record should write");

    let error = GitProvenanceRecord::load(&history, 0)
        .expect_err("tampered record should fail verification");

    assert!(matches!(
        error,
        GitProvenanceError::RecordDigestMismatch { .. }
    ),);

    fs::remove_dir_all(root).expect("test should clean up");
}

#[test]
fn conflicting_second_binding_is_rejected() {
    let root = temporary_directory("conflict");

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("baseline", &report())
        .expect("history should append");

    let run = history.latest().expect("history should contain run");

    let first =
        GitProvenanceRecord::captured_clean(run, "6".repeat(40), "7".repeat(40), Vec::new())
            .expect("first record should build");

    first
        .persist(&history)
        .expect("first record should persist");

    let second =
        GitProvenanceRecord::user_asserted(run, "8".repeat(40), "9".repeat(40), Vec::new())
            .expect("second record should build");

    let error = second
        .persist(&history)
        .expect_err("conflicting binding must be rejected");

    assert!(matches!(error, GitProvenanceError::AlreadyExists { .. }),);

    fs::remove_dir_all(root).expect("test should clean up");
}
