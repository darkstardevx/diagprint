use diagprint::{
    DIAGNOSTIC_CASE_FILE_V1_SCHEMA, DiagnosticCaseStatus, DiagnosticHistory, DiagnosticReport,
    Reporter,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagnostic-forensics-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-forensics-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn issue_report(severity: &str, instances: usize) -> DiagnosticReport {
    let reporter = reporter();

    let mut report = DiagnosticReport::new();

    for _ in 0..instances {
        let diagnostic = match severity {
            "warning" => reporter.warning("configuration drift").code("W100"),

            "error" => reporter.error("configuration drift").code("W100"),

            other => {
                panic!("unsupported severity {other}");
            }
        };

        report.push(diagnostic);
    }

    report
}

#[test]
fn case_file_reconstructs_episodes_and_reappearance() {
    let root = temporary_directory("episodes");

    let warning = issue_report("warning", 1);

    let fingerprint = warning
        .iter()
        .next()
        .expect("warning report should contain one diagnostic")
        .fingerprint()
        .qualified();

    let error = issue_report("error", 1);

    assert_eq!(
        error
            .iter()
            .next()
            .expect("error report should contain one diagnostic",)
            .fingerprint()
            .qualified(),
        fingerprint,
        "severity changes must retain logical fingerprint identity",
    );

    let duplicate_warning = issue_report("warning", 2);

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("baseline", &warning)
        .expect("baseline should append");

    history
        .append_report("severity-regression", &error)
        .expect("severity regression should append");

    history
        .append_report("fixed", &DiagnosticReport::new())
        .expect("fixed state should append");

    history
        .append_report("still-clean", &DiagnosticReport::new())
        .expect("clean state should append");

    history
        .append_report("regressed", &duplicate_warning)
        .expect("regression should append");

    history.verify().expect("history should verify");

    let case = history
        .case_file(&fingerprint)
        .expect("fingerprint should produce case file");

    assert_eq!(case.schema, DIAGNOSTIC_CASE_FILE_V1_SCHEMA,);

    assert_eq!(case.status, DiagnosticCaseStatus::Active,);

    assert!(case.is_active(),);

    assert_eq!(case.history_runs, 5,);

    assert_eq!(case.observed_runs, 3,);

    assert_eq!(case.observed_instances, 4,);

    assert_eq!(case.unique_digests, 2,);

    assert_eq!(case.first_seen_run, 0,);

    assert_eq!(case.last_seen_run, 4,);

    assert_eq!(case.active_instances, 2,);

    assert_eq!(case.introduced_instances, 3,);

    assert_eq!(case.resolved_instances, 1,);

    assert_eq!(case.changed_instances, 1,);

    assert_eq!(case.severity_increases, 1,);

    assert_eq!(case.reappearances, 1,);

    assert_eq!(case.episodes.len(), 2,);

    let first = &case.episodes[0];

    assert_eq!(first.episode, 1,);

    assert_eq!(first.started_run, 0,);

    assert_eq!(first.last_active_run, 1,);

    assert_eq!(first.resolved_run, Some(2),);

    assert_eq!(first.observed_runs, 2,);

    assert_eq!(first.instances, 2,);

    assert_eq!(first.peak_instances, 1,);

    let second = &case.episodes[1];

    assert_eq!(second.episode, 2,);

    assert_eq!(second.started_run, 4,);

    assert_eq!(second.last_active_run, 4,);

    assert_eq!(second.resolved_run, None,);

    assert_eq!(second.observed_runs, 1,);

    assert_eq!(second.instances, 2,);

    assert_eq!(second.peak_instances, 2,);

    assert_eq!(case.evidence[0].label, "baseline",);

    assert_eq!(case.evidence[1].label, "severity-regression",);

    assert_eq!(case.evidence[2].label, "regressed",);

    assert_eq!(case.evidence[2].severities.get("warning"), Some(&2),);

    assert!(
        case.chain_head
            .as_deref()
            .is_some_and(|head| { head.starts_with("sha256:",) },),
    );

    let json = serde_json::to_string(&case).expect("case file should serialize");

    assert!(
        !json.contains("configuration drift",),
        "privacy-light case file must not regain diagnostic message text",
    );

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn resolved_case_reports_closed_episode() {
    let root = temporary_directory("resolved");

    let report = issue_report("warning", 1);

    let fingerprint = report
        .iter()
        .next()
        .expect("report should contain a diagnostic")
        .fingerprint()
        .qualified();

    let mut history = DiagnosticHistory::open(&root).expect("history should open");

    history
        .append_report("present", &report)
        .expect("present run should append");

    history
        .append_report("resolved", &DiagnosticReport::new())
        .expect("resolved run should append");

    let case = history.case_file(&fingerprint).expect("case should exist");

    assert_eq!(case.status, DiagnosticCaseStatus::Resolved,);

    assert!(case.is_resolved(),);

    assert_eq!(case.active_instances, 0,);

    assert_eq!(case.reappearances, 0,);

    assert_eq!(case.episodes.len(), 1,);

    assert_eq!(case.episodes[0].resolved_run, Some(1),);

    fs::remove_dir_all(root).expect("history should clean up");
}

#[test]
fn unknown_fingerprint_has_no_case_file() {
    let root = temporary_directory("unknown");

    let history = DiagnosticHistory::open(&root).expect("history should open");

    assert!(
        history
            .case_file("diagprint.canonical/v1:sha256:deadbeef",)
            .is_none(),
    );

    fs::remove_dir_all(root).expect("history should clean up");
}
