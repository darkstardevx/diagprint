use diagprint::{
    Applicability, DIAGNOSTIC_REMEDIATION_REPLAY_V1_SCHEMA, DiagnosticHistory,
    DiagnosticRemediationState, DiagnosticReport, Edit, FileCheck, FixPlan, FixPlanReport,
    RemediationReceipt, Reporter, TextRange,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("remediation-replay-test")
        .color(false)
        .build()
        .unwrap()
}

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-remediation-replay-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn target(message: &str) -> DiagnosticReport {
    DiagnosticReport::from_diagnostic(
        reporter()
            .error(message)
            .code("M5-REPLAY")
            .attribute("diagprint.identity", "m5.replay.target"),
    )
}

fn target_twice(message: &str) -> DiagnosticReport {
    let diagnostic = reporter()
        .error(message)
        .code("M5-REPLAY")
        .attribute("diagprint.identity", "m5.replay.target");

    let mut report = DiagnosticReport::new();
    report.push(diagnostic.clone());
    report.push(diagnostic);
    report
}

fn absent() -> DiagnosticReport {
    DiagnosticReport::new()
}

fn fingerprint(report: &DiagnosticReport) -> String {
    report.iter().next().unwrap().fingerprint().qualified()
}

fn receipt(
    before: &DiagnosticReport,
    after: &DiagnosticReport,
    verified: bool,
    tag: &str,
) -> RemediationReceipt {
    let mut plan = FixPlan::new(format!("private plan {tag}"))
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            format!("private-{tag}.txt"),
            TextRange::new(0, 1),
            "a",
            "b",
        ));

    if verified {
        plan = plan.verify(FileCheck::exists(format!("verify-{tag}.txt")));
    }

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from(format!("private-{tag}.txt"))],
        verification_checks: usize::from(verified),
        verification_passed: true,
    };

    RemediationReceipt::from_successful_apply(before, after, &plan, &applied).unwrap()
}

fn append_all(history: &mut DiagnosticHistory, reports: &[&DiagnosticReport]) {
    for (index, report) in reports.iter().enumerate() {
        history
            .append_report(format!("run-{index}"), report)
            .unwrap();
    }
}

#[test]
fn replay_classifies_all_immediate_states_with_multiset_semantics() {
    let root = temporary_directory("states");

    let r0 = absent();
    let r1 = target("stable");
    let r2 = target("stable");
    let r3 = target_twice("stable");
    let r4 = target("changed wording");
    let r5 = absent();
    let r6 = absent();
    let r7 = target("later");

    let fp = fingerprint(&r1);

    let mut history = DiagnosticHistory::open(root.join("history")).unwrap();

    append_all(&mut history, &[&r0, &r1, &r2, &r3, &r4, &r5, &r6, &r7]);

    for (before, after, left, right) in [
        (0, 1, &r0, &r1),
        (1, 2, &r1, &r2),
        (2, 3, &r2, &r3),
        (3, 4, &r3, &r4),
        (4, 5, &r4, &r5),
        (5, 6, &r5, &r6),
    ] {
        history
            .record_remediation_evidence(
                before,
                after,
                &receipt(left, right, false, &format!("{before}-{after}")),
            )
            .unwrap();
    }

    let replay = history.remediation_replay(&fp).unwrap();

    assert_eq!(replay.schema, DIAGNOSTIC_REMEDIATION_REPLAY_V1_SCHEMA);
    assert!(replay.history_chain_verified);
    assert!(replay.remediation_records_verified);
    assert_eq!(replay.steps.len(), 6);

    let states = replay
        .steps
        .iter()
        .map(|step| step.observed_transition)
        .collect::<Vec<_>>();

    assert_eq!(
        states,
        vec![
            DiagnosticRemediationState::Introduced,
            DiagnosticRemediationState::Persisting,
            DiagnosticRemediationState::Changed,
            DiagnosticRemediationState::Changed,
            DiagnosticRemediationState::Resolved,
            DiagnosticRemediationState::Absent,
        ]
    );

    let resolved = &replay.steps[4];

    assert_eq!(resolved.before_instances, 1);
    assert_eq!(resolved.after_instances, 0);
    assert_eq!(resolved.later_reappearance_run, Some(7));
    assert!(resolved.assessment.observed_resolution);
    assert!(resolved.assessment.later_reappearance_observed);
    assert!(!resolved.assessment.post_apply_verification);
    assert!(!resolved.assessment.regression_after_verified_remediation);

    assert_eq!(replay.verified_regressions, 0);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verified_resolution_then_reappearance_is_observed_as_verified_regression() {
    let root = temporary_directory("verified-regression");

    let r0 = target("before");
    let r1 = absent();
    let r2 = absent();
    let r3 = target("returned");

    let fp = fingerprint(&r0);

    let mut history = DiagnosticHistory::open(root.join("history")).unwrap();
    append_all(&mut history, &[&r0, &r1, &r2, &r3]);

    history
        .record_remediation_evidence(0, 1, &receipt(&r0, &r1, true, "verified"))
        .unwrap();

    let replay = history.remediation_replay(&fp).unwrap();
    let step = &replay.steps[0];

    assert_eq!(
        step.observed_transition,
        DiagnosticRemediationState::Resolved
    );
    assert_eq!(step.remediation_status, "verified");
    assert_eq!(step.later_reappearance_run, Some(3));

    assert!(step.assessment.evidence_record_verified);
    assert!(step.assessment.post_apply_verification);
    assert!(step.assessment.observed_resolution);
    assert!(step.assessment.later_reappearance_observed);
    assert!(step.assessment.regression_after_verified_remediation);

    assert!(!step.assessment.remediation_caused_resolution_established);
    assert!(!step.assessment.recurrence_root_cause_established);
    assert!(!step.assessment.git_causation_established);

    assert_eq!(replay.verified_regressions, 1);
    assert!(replay.has_verified_regression());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn persisting_target_never_becomes_false_regression() {
    let root = temporary_directory("persisting");

    let r0 = target("same");
    let r1 = target("same");
    let r2 = target("same");

    let fp = fingerprint(&r0);

    let mut history = DiagnosticHistory::open(root.join("history")).unwrap();
    append_all(&mut history, &[&r0, &r1, &r2]);

    history
        .record_remediation_evidence(0, 1, &receipt(&r0, &r1, true, "persisting"))
        .unwrap();

    let replay = history.remediation_replay(&fp).unwrap();
    let step = &replay.steps[0];

    assert_eq!(
        step.observed_transition,
        DiagnosticRemediationState::Persisting
    );
    assert_eq!(step.later_reappearance_run, None);
    assert!(!step.assessment.observed_resolution);
    assert!(!step.assessment.regression_after_verified_remediation);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn multiple_remediation_records_replay_in_chronological_order() {
    let root = temporary_directory("multiple");

    let r0 = target("first");
    let r1 = absent();
    let r2 = target("second");
    let r3 = target("second changed");

    let fp = fingerprint(&r0);

    let mut history = DiagnosticHistory::open(root.join("history")).unwrap();
    append_all(&mut history, &[&r0, &r1, &r2, &r3]);

    history
        .record_remediation_evidence(0, 1, &receipt(&r0, &r1, true, "first"))
        .unwrap();

    history
        .record_remediation_evidence(2, 3, &receipt(&r2, &r3, false, "second"))
        .unwrap();

    let replay = history.remediation_replay(&fp).unwrap();

    assert_eq!(replay.steps.len(), 2);
    assert_eq!(
        (replay.steps[0].before_run, replay.steps[0].after_run),
        (0, 1)
    );
    assert_eq!(
        (replay.steps[1].before_run, replay.steps[1].after_run),
        (2, 3)
    );

    assert_eq!(
        replay.steps[0].observed_transition,
        DiagnosticRemediationState::Resolved
    );
    assert_eq!(
        replay.steps[1].observed_transition,
        DiagnosticRemediationState::Changed
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_json_is_deterministic_and_privacy_light() {
    let root = temporary_directory("json");

    let r0 = target("SECRET_DIAGNOSTIC_REPLAY_MESSAGE");
    let r1 = absent();

    let fp = fingerprint(&r0);

    let mut history = DiagnosticHistory::open(root.join("history")).unwrap();
    append_all(&mut history, &[&r0, &r1]);

    history
        .record_remediation_evidence(0, 1, &receipt(&r0, &r1, true, "SECRET_REPLAY_PLAN"))
        .unwrap();

    let first = serde_json::to_string_pretty(&history.remediation_replay(&fp).unwrap()).unwrap();

    let second = serde_json::to_string_pretty(&history.remediation_replay(&fp).unwrap()).unwrap();

    assert_eq!(first, second);

    for secret in [
        "SECRET_DIAGNOSTIC_REPLAY_MESSAGE",
        "SECRET_REPLAY_PLAN",
        "private-SECRET_REPLAY_PLAN.txt",
        "verify-SECRET_REPLAY_PLAN.txt",
    ] {
        assert!(!first.contains(secret));
    }

    assert!(first.contains("\"remediation_caused_resolution_established\": false"));
    assert!(first.contains("\"recurrence_root_cause_established\": false"));
    assert!(first.contains("\"git_causation_established\": false"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_requires_exact_known_fingerprint_at_api_boundary() {
    let root = temporary_directory("unknown");

    let report = target("known");
    let mut history = DiagnosticHistory::open(root.join("history")).unwrap();
    history.append_report("known", &report).unwrap();

    let error = history
        .remediation_replay(
            "diagprint.canonical/v1:sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        )
        .expect_err("unknown exact fingerprint must fail");

    assert!(error.to_string().contains("not present in this history"));

    fs::remove_dir_all(root).unwrap();
}
