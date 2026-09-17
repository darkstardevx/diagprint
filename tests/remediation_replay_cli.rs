use diagprint::{
    Applicability, DiagnosticHistory, DiagnosticReport, Edit, FileCheck, FixPlan, FixPlanReport,
    RemediationReceipt, Reporter, TextRange,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("remediation-replay-cli-test")
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
        "diagprint-remediation-replay-cli-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn target(message: &str) -> DiagnosticReport {
    DiagnosticReport::from_diagnostic(
        reporter()
            .error(message)
            .code("M5-CLI")
            .attribute("diagprint.identity", "m5.cli.target"),
    )
}

fn absent() -> DiagnosticReport {
    DiagnosticReport::new()
}

fn receipt(before: &DiagnosticReport, after: &DiagnosticReport) -> RemediationReceipt {
    let plan = FixPlan::new("CLI_PRIVATE_PLAN")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "CLI_PRIVATE_SOURCE.txt",
            TextRange::new(0, 1),
            "x",
            "y",
        ))
        .verify(FileCheck::exists("CLI_PRIVATE_VERIFY.txt"));

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from("CLI_PRIVATE_SOURCE.txt")],
        verification_checks: 1,
        verification_passed: true,
    };

    RemediationReceipt::from_successful_apply(before, after, &plan, &applied).unwrap()
}

fn fixture(name: &str) -> (PathBuf, String) {
    let root = temporary_directory(name);
    let history_path = root.join("history");

    let before = target("CLI_PRIVATE_DIAGNOSTIC");
    let after = absent();
    let later_absent = absent();
    let reappeared = target("returned");

    let fingerprint = before.iter().next().unwrap().fingerprint().qualified();

    let mut history = DiagnosticHistory::open(&history_path).unwrap();

    history.append_report("before", &before).unwrap();
    history.append_report("after", &after).unwrap();
    history
        .append_report("still absent", &later_absent)
        .unwrap();
    history.append_report("reappeared", &reappeared).unwrap();

    history
        .record_remediation_evidence(0, 1, &receipt(&before, &after))
        .unwrap();

    (root, fingerprint)
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diagprint"))
        .args(args)
        .output()
        .expect("diagprint binary should run")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn history_text(root: &Path) -> String {
    root.join("history").to_string_lossy().into_owned()
}

#[test]
fn replay_text_and_history_alias_report_evidence_boundaries() {
    let (root, fingerprint) = fixture("text");
    let history = history_text(&root);

    let hex = fingerprint.rsplit(':').next().unwrap();
    let short = &hex[..12];

    let direct = run(&["replay", &history, short]);

    assert!(
        direct.status.success(),
        "direct replay failed:\n{}",
        output_text(&direct)
    );

    let text = output_text(&direct);

    assert!(text.contains("DIAGNOSTIC REMEDIATION REPLAY"));
    assert!(text.contains("history-chain-verified: true"));
    assert!(text.contains("remediation-records-verified: true"));
    assert!(text.contains("observed-transition: resolved"));
    assert!(text.contains("later-reappearance: run=000003"));
    assert!(text.contains("regression-after-verified-remediation: OBSERVED"));
    assert!(text.contains("remediation-caused-resolution: NOT ESTABLISHED"));
    assert!(text.contains("recurrence-root-cause: NOT ESTABLISHED"));
    assert!(text.contains("git-causation: NOT ESTABLISHED"));

    for secret in [
        "CLI_PRIVATE_PLAN",
        "CLI_PRIVATE_SOURCE",
        "CLI_PRIVATE_VERIFY",
        "CLI_PRIVATE_DIAGNOSTIC",
    ] {
        assert!(!text.contains(secret));
    }

    let alias = run(&["history", "replay", &history, short]);

    assert!(
        alias.status.success(),
        "history replay failed:\n{}",
        output_text(&alias)
    );

    assert!(output_text(&alias).contains("DIAGNOSTIC REMEDIATION REPLAY"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_json_is_deterministic_and_semantic() {
    let (root, fingerprint) = fixture("json");
    let history = history_text(&root);

    let first = run(&["replay", &history, &fingerprint, "--format", "json"]);

    let second = run(&["replay", &history, &fingerprint, "--format", "json"]);

    assert!(first.status.success(), "{}", output_text(&first));
    assert!(second.status.success(), "{}", output_text(&second));

    assert_eq!(first.stdout, second.stdout);

    let value: Value = serde_json::from_slice(&first.stdout).expect("JSON should parse");

    assert_eq!(value["schema"], "diagprint.forensics.remediation-replay/v1");
    assert_eq!(value["fingerprint"], fingerprint);
    assert_eq!(value["verified_regressions"], 1);
    assert_eq!(value["steps"][0]["observed_transition"], "resolved");
    assert_eq!(value["steps"][0]["later_reappearance_run"], 3);
    assert_eq!(
        value["steps"][0]["assessment"]["regression_after_verified_remediation"],
        true
    );
    assert_eq!(
        value["steps"][0]["assessment"]["remediation_caused_resolution_established"],
        false
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn remediation_verify_checks_all_sidecars() {
    let (root, _) = fixture("verify");
    let history = history_text(&root);

    let output = run(&["history", "remediation-verify", &history]);

    assert!(output.status.success(), "{}", output_text(&output));

    let text = output_text(&output);
    assert!(text.contains("REMEDIATION EVIDENCE VERIFIED"));
    assert!(text.contains("records: 1"));
    assert!(text.contains("status=verified"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_fails_before_presentation_when_evidence_is_tampered() {
    let (root, fingerprint) = fixture("tamper");
    let history = history_text(&root);

    let evidence_path = root
        .join("history")
        .join("remediation")
        .join("run-000001.json");

    let mut value: Value = serde_json::from_slice(&fs::read(&evidence_path).unwrap()).unwrap();

    value["changed_files"] = Value::from(999_u64);

    fs::write(&evidence_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let output = run(&["replay", &history, &fingerprint]);

    assert!(!output.status.success());

    let text = output_text(&output);

    assert!(text.contains("digest mismatch"));
    assert!(!text.contains("DIAGNOSTIC REMEDIATION REPLAY"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_is_read_only_for_history_and_evidence_bytes() {
    let (root, fingerprint) = fixture("read-only");
    let history_path = root.join("history");
    let history = history_path.to_string_lossy().into_owned();

    let run0 = history_path.join("run-000000.json");
    let run1 = history_path.join("run-000001.json");
    let head = history_path.join("head.json");
    let evidence = history_path.join("remediation").join("run-000001.json");

    let before = [
        fs::read(&run0).unwrap(),
        fs::read(&run1).unwrap(),
        fs::read(&head).unwrap(),
        fs::read(&evidence).unwrap(),
    ];

    let output = run(&["replay", &history, &fingerprint]);

    assert!(output.status.success(), "{}", output_text(&output));

    let after = [
        fs::read(&run0).unwrap(),
        fs::read(&run1).unwrap(),
        fs::read(&head).unwrap(),
        fs::read(&evidence).unwrap(),
    ];

    assert_eq!(before, after);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replay_rejects_unknown_fingerprint_prefix() {
    let (root, _) = fixture("unknown");
    let history = history_text(&root);

    let output = run(&["replay", &history, "deadbeefdead"]);

    assert!(!output.status.success());
    assert!(output_text(&output).contains("no diagnostic fingerprint matches"));

    fs::remove_dir_all(root).unwrap();
}
