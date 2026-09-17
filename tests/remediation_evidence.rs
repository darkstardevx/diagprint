use diagprint::{
    Applicability, DiagnosticHistory, DiagnosticReport, Edit, FileCheck, FixPlan, FixPlanReport,
    REMEDIATION_EVIDENCE_V1_SCHEMA, RemediationEvidenceError, RemediationReceipt, Reporter,
    TextRange,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("remediation-evidence-test")
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
        "diagprint-remediation-evidence-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent should be created");
    }

    fs::write(path, contents).expect("test file should be written");
}

fn before_report(message: &str) -> DiagnosticReport {
    DiagnosticReport::from_diagnostic(
        reporter()
            .error(message)
            .code("M5-TARGET")
            .attribute("diagprint.identity", "m5.target"),
    )
}

fn after_report() -> DiagnosticReport {
    DiagnosticReport::new()
}

struct Fixture {
    root: PathBuf,
    history: DiagnosticHistory,
    before: DiagnosticReport,
    after: DiagnosticReport,
    receipt: RemediationReceipt,
}

impl Fixture {
    fn verified(name: &str) -> Self {
        let root = temporary_directory(name);
        let history_dir = root.join("history");
        let file = root.join("private-source.txt");

        write(&file, "secret-old");

        let before = before_report("SECRET_DIAGNOSTIC_MESSAGE");
        let after = after_report();

        let plan = FixPlan::new("SECRET_PLAN_TITLE")
            .applicability(Applicability::MachineApplicable)
            .precondition(FileCheck::equals(file.clone(), "secret-old"))
            .edit(Edit::replace(
                file.clone(),
                TextRange::new(0, "secret-old".len()),
                "secret-old",
                "secret-new",
            ))
            .verify(FileCheck::equals(file.clone(), "secret-new"));

        let applied = plan.apply().expect("fix plan should apply");

        let receipt = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
            .expect("receipt should build");

        let mut history = DiagnosticHistory::open(&history_dir).expect("history should open");

        history
            .append_report("before remediation", &before)
            .expect("before run should append");

        history
            .append_report("after remediation", &after)
            .expect("after run should append");

        Self {
            root,
            history,
            before,
            after,
            receipt,
        }
    }

    fn cleanup(self) {
        fs::remove_dir_all(self.root).expect("fixture should clean up");
    }

    fn evidence_path(&self) -> PathBuf {
        self.history
            .directory()
            .join("remediation")
            .join("run-000001.json")
    }
}

#[test]
fn valid_receipt_binds_exact_adjacent_history_transition() {
    let fixture = Fixture::verified("valid");

    let record = fixture
        .history
        .record_remediation_evidence(0, 1, &fixture.receipt)
        .expect("evidence should record");

    assert_eq!(record.schema, REMEDIATION_EVIDENCE_V1_SCHEMA);
    assert_eq!(record.before_run, 0);
    assert_eq!(record.after_run, 1);

    assert_eq!(
        record.before_run_digest,
        fixture.history.runs()[0].run_digest
    );
    assert_eq!(
        record.after_run_digest,
        fixture.history.runs()[1].run_digest
    );

    assert_eq!(
        record.before_report_digest,
        fixture.history.runs()[0].report_digest
    );
    assert_eq!(
        record.after_report_digest,
        fixture.history.runs()[1].report_digest
    );

    assert_eq!(
        record.plan_descriptor_digest,
        fixture.receipt.plan.descriptor_digest
    );

    assert_eq!(record.remediation_status, "verified");
    assert!(record.verification_passed);

    assert_eq!(record.effect.before_diagnostics, 1);
    assert_eq!(record.effect.after_diagnostics, 0);
    assert_eq!(record.effect.resolved, 1);
    assert_eq!(record.effect.new, 0);

    fixture
        .history
        .verify_remediation_evidence()
        .expect("persisted evidence should verify");

    fixture.cleanup();
}

#[test]
fn identical_retry_is_idempotent() {
    let fixture = Fixture::verified("idempotent");

    let first = fixture
        .history
        .record_remediation_evidence(0, 1, &fixture.receipt)
        .expect("first evidence write should succeed");

    let second = fixture
        .history
        .record_remediation_evidence(0, 1, &fixture.receipt)
        .expect("identical retry should succeed");

    assert_eq!(first, second);

    let records = fixture
        .history
        .remediation_evidence_records()
        .expect("records should load");

    assert_eq!(records.len(), 1);

    fixture.cleanup();
}

#[test]
fn non_adjacent_history_binding_is_rejected() {
    let root = temporary_directory("non-adjacent");
    let history_dir = root.join("history");

    let before = before_report("target");
    let middle = before_report("target changed");
    let after = after_report();

    let mut history = DiagnosticHistory::open(&history_dir).expect("history should open");

    history
        .append_report("before", &before)
        .expect("run 0 should append");

    history
        .append_report("middle", &middle)
        .expect("run 1 should append");

    history
        .append_report("after", &after)
        .expect("run 2 should append");

    let plan = FixPlan::new("plan")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace("unused.txt", TextRange::new(0, 1), "a", "b"));

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from("unused.txt")],
        verification_checks: 0,
        verification_passed: true,
    };

    let receipt = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
        .expect("receipt should build");

    let error = history
        .record_remediation_evidence(0, 2, &receipt)
        .expect_err("non-adjacent evidence must fail");

    assert!(matches!(
        error,
        RemediationEvidenceError::NonAdjacentTransition {
            before_run: 0,
            after_run: 2
        }
    ));

    fs::remove_dir_all(root).expect("fixture should clean up");
}

#[test]
fn receipt_report_mismatch_is_rejected() {
    let fixture = Fixture::verified("report-mismatch");

    let other_before = before_report("different logical report content");

    let plan = FixPlan::new("other")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace("unused.txt", TextRange::new(0, 1), "a", "b"));

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from("unused.txt")],
        verification_checks: 0,
        verification_passed: true,
    };

    let wrong_receipt =
        RemediationReceipt::from_successful_apply(&other_before, &fixture.after, &plan, &applied)
            .expect("alternate receipt should build");

    let error = fixture
        .history
        .record_remediation_evidence(0, 1, &wrong_receipt)
        .expect_err("mismatched report receipt must fail");

    assert!(matches!(
        error,
        RemediationEvidenceError::ReceiptBeforeReportMismatch { .. }
    ));

    fixture.cleanup();
}

#[test]
fn aggregate_effect_mismatch_is_rejected() {
    let fixture = Fixture::verified("effect-mismatch");
    let mut receipt = fixture.receipt.clone();

    receipt.effect.resolved += 1;

    let error = fixture
        .history
        .record_remediation_evidence(0, 1, &receipt)
        .expect_err("mismatched semantic effect must fail");

    assert!(matches!(
        error,
        RemediationEvidenceError::EffectMismatch { .. }
    ));

    fixture.cleanup();
}

#[test]
fn conflicting_record_for_same_after_run_is_rejected() {
    let fixture = Fixture::verified("conflict");

    fixture
        .history
        .record_remediation_evidence(0, 1, &fixture.receipt)
        .expect("first evidence should record");

    let plan = FixPlan::new("different private title")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "other-private.txt",
            TextRange::new(0, 1),
            "a",
            "b",
        ));

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from("other-private.txt")],
        verification_checks: 0,
        verification_passed: true,
    };

    let alternate =
        RemediationReceipt::from_successful_apply(&fixture.before, &fixture.after, &plan, &applied)
            .expect("alternate receipt should build");

    let error = fixture
        .history
        .record_remediation_evidence(0, 1, &alternate)
        .expect_err("different evidence for same transition must conflict");

    assert!(matches!(
        error,
        RemediationEvidenceError::ConflictingRecord { after_run: 1, .. }
    ));

    fixture.cleanup();
}

#[test]
fn tampered_record_digest_is_detected() {
    let fixture = Fixture::verified("tamper");

    fixture
        .history
        .record_remediation_evidence(0, 1, &fixture.receipt)
        .expect("evidence should record");

    let path = fixture.evidence_path();
    let bytes = fs::read(&path).expect("evidence should be readable");
    let mut value: Value = serde_json::from_slice(&bytes).expect("evidence JSON should parse");

    value["effect"]["resolved"] = Value::from(99_u64);

    fs::write(
        &path,
        serde_json::to_vec_pretty(&value).expect("tampered JSON should serialize"),
    )
    .expect("tampered evidence should write");

    let error = fixture
        .history
        .verify_remediation_evidence()
        .expect_err("tampered evidence must fail verification");

    assert!(matches!(
        error,
        RemediationEvidenceError::RecordDigestMismatch { .. }
    ));

    fixture.cleanup();
}

#[test]
fn evidence_sidecar_excludes_private_plan_source_and_diagnostic_text() {
    let fixture = Fixture::verified("privacy");

    fixture
        .history
        .record_remediation_evidence(0, 1, &fixture.receipt)
        .expect("evidence should record");

    let json = fs::read_to_string(fixture.evidence_path()).expect("evidence should be readable");

    for secret in [
        "SECRET_PLAN_TITLE",
        "SECRET_DIAGNOSTIC_MESSAGE",
        "private-source.txt",
        "secret-old",
        "secret-new",
        &fixture.root.display().to_string(),
    ] {
        assert!(
            !json.contains(secret),
            "evidence sidecar leaked private sentinel {secret:?}"
        );
    }

    assert!(json.contains("receipt_digest"));
    assert!(json.contains("plan_descriptor_digest"));
    assert!(json.contains("\"resolved\": 1"));

    fixture.cleanup();
}
