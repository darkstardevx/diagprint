use diagprint::{
    Applicability, DELTA_V1_SCHEMA, DeltaPolicy, DiagnosticReport, Edit, ExportPolicy, Reporter,
    Severity, SuggestedCommand, Suggestion, TextRange,
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("delta-artifact-test")
        .build()
        .unwrap()
}

fn report(diagnostics: impl IntoIterator<Item = diagprint::Diagnostic>) -> DiagnosticReport {
    diagnostics.into_iter().collect()
}

#[test]
fn artifact_records_schema_report_identity_and_counts() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same").code("A100")]);

    let candidate = report([
        reporter.error("same").code("A100"),
        reporter.error("new").code("A101"),
    ]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let artifact = delta.artifact(&ExportPolicy::default());

    assert_eq!(artifact.schema, DELTA_V1_SCHEMA);
    assert_eq!(artifact.baseline_report, delta.baseline_digest());
    assert_eq!(artifact.candidate_report, delta.candidate_digest());

    assert_eq!(artifact.counts.new, 1);
    assert_eq!(artifact.counts.changed, 1);
    assert_eq!(artifact.counts.baseline_total, 1);
    assert_eq!(artifact.counts.candidate_total, 2);

    assert_eq!(artifact.entries.len(), 2);
    assert!(artifact.evaluation.is_none());
}

#[test]
fn evaluated_artifact_records_exact_ci_boundary() {
    let reporter = reporter();

    let baseline = report([reporter.warning("promoted").code("A200")]);

    let candidate = report([
        reporter.error("promoted").code("A200"),
        reporter.error("new").code("A201"),
    ]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let policy = DeltaPolicy::ci();

    let artifact = delta.evaluated_artifact(&policy, &ExportPolicy::default());

    let evaluation = artifact
        .evaluation
        .as_ref()
        .expect("evaluated artifact should contain evaluation");

    assert_eq!(evaluation.status, "failure");
    assert_eq!(evaluation.exit_code, 1);

    assert_eq!(evaluation.policy.new_at_or_above, Some(Severity::Error));

    assert_eq!(
        evaluation.policy.severity_increase_at_or_above,
        Some(Severity::Error)
    );

    assert_eq!(evaluation.violations.len(), 2);
}

#[test]
fn default_artifact_export_does_not_leak_sensitive_payloads() {
    let reporter = reporter();

    let suggestion = Suggestion::new("repair")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "/home/alice/private/project/src/main.rs",
            TextRange::new(0, 3),
            "secret-old-value",
            "secret-new-value",
        ))
        .command(SuggestedCommand::new("super-secret-follow-up-command"));

    let candidate = report([reporter
        .error("visible diagnostic")
        .code("A300")
        .attribute("authorization", "Bearer super-secret-token")
        .label(
            "/home/alice/private/project/src/main.rs",
            10,
            Some(2),
            Some(3),
            Some("visible label"),
        )
        .suggestion(suggestion)]);

    let baseline = DiagnosticReport::new();

    let delta = candidate.delta_from(&baseline).unwrap();

    let json = delta
        .artifact(&ExportPolicy::default())
        .to_json_pretty()
        .unwrap();

    assert!(json.contains("main.rs"));

    assert!(!json.contains("/home/alice/private/project/src/main.rs"));

    assert!(!json.contains("Bearer super-secret-token"));
    assert!(!json.contains("secret-old-value"));
    assert!(!json.contains("secret-new-value"));
    assert!(!json.contains("super-secret-follow-up-command"));

    assert!(json.contains("\"edit_count\": 1"));
    assert!(json.contains("\"command_count\": 1"));
}

#[test]
fn artifact_serialization_is_independent_of_report_insertion_order() {
    let reporter = reporter();

    let baseline_one = reporter.warning("one").code("A400");
    let baseline_two = reporter.error("two").code("A401");

    let candidate_one = reporter.error("one").code("A400");
    let candidate_two = reporter.error("three").code("A402");

    let baseline_a = report([baseline_one.clone(), baseline_two.clone()]);

    let baseline_b = report([baseline_two, baseline_one]);

    let candidate_a = report([candidate_one.clone(), candidate_two.clone()]);

    let candidate_b = report([candidate_two, candidate_one]);

    let delta_a = candidate_a.delta_from(&baseline_a).unwrap();
    let delta_b = candidate_b.delta_from(&baseline_b).unwrap();

    let json_a = delta_a
        .artifact(&ExportPolicy::default())
        .to_json_pretty()
        .unwrap();

    let json_b = delta_b
        .artifact(&ExportPolicy::default())
        .to_json_pretty()
        .unwrap();

    assert_eq!(json_a, json_b);
}

#[test]
fn artifact_fingerprint_policy_is_self_describing() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter.warning("diagnostic").code("A500")]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let artifact = delta.artifact(&ExportPolicy::default());

    assert!(artifact.fingerprint_policy.include_code);
    assert!(artifact.fingerprint_policy.include_message);

    assert_eq!(artifact.fingerprint_policy.source, "file_name");

    assert!(artifact.fingerprint_policy.include_primary_label_message);

    assert!(artifact.fingerprint_policy.prefer_explicit_identity);
}
