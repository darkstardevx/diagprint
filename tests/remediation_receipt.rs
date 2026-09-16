use diagprint::{
    Applicability, DiagnosticReport, Edit, FIX_PLAN_DESCRIPTOR_V1_SCHEMA, FileCheck, FixPlan,
    FixPlanReport, REMEDIATION_RECEIPT_V1_SCHEMA, RemediationReceipt, RemediationReceiptError,
    RemediationStatus, Reporter, TextRange,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("remediation-receipt-test")
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
        "diagprint-remediation-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent should be created");
    }

    fs::write(path, contents).expect("test file should be written");
}

#[test]
fn fix_plan_descriptor_captures_complete_plan_shape() {
    let path = PathBuf::from("src/config.txt");

    let plan = FixPlan::new("replace configuration value")
        .explanation("replace the guarded value")
        .applicability(Applicability::MachineApplicable)
        .precondition(FileCheck::contains(path.clone(), "before"))
        .edit(Edit::replace(
            path.clone(),
            TextRange::new(0, 6),
            "before",
            "after",
        ))
        .verify(FileCheck::contains(path, "after"))
        .backups(true)
        .backup_suffix(".test-backup");

    let descriptor = plan.descriptor();

    assert_eq!(descriptor.schema, FIX_PLAN_DESCRIPTOR_V1_SCHEMA,);

    assert_eq!(descriptor.title, "replace configuration value",);

    assert_eq!(descriptor.applicability, Applicability::MachineApplicable,);

    assert_eq!(descriptor.preconditions.len(), 1,);

    assert_eq!(descriptor.edits.len(), 1,);

    assert_eq!(descriptor.verifications.len(), 1,);

    assert!(descriptor.backups);

    assert_eq!(descriptor.backup_suffix, ".test-backup",);
}

#[test]
fn remediation_receipt_records_verified_effect() {
    let root = temporary_directory("verified");

    let file = root.join("config.txt");

    write(&file, "before");

    let before = DiagnosticReport::from_diagnostic(
        reporter().error("configuration is invalid").code("R100"),
    );

    let after = DiagnosticReport::new();

    let plan = FixPlan::new("repair configuration")
        .applicability(Applicability::MachineApplicable)
        .precondition(FileCheck::equals(file.clone(), "before"))
        .edit(Edit::replace(
            file.clone(),
            TextRange::new(0, 6),
            "before",
            "after",
        ))
        .verify(FileCheck::equals(file.clone(), "after"));

    let applied = plan.apply().expect("fix plan should apply");

    assert_eq!(
        fs::read_to_string(&file).expect("changed file should be readable",),
        "after",
    );

    let receipt = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
        .expect("receipt should be created");

    assert_eq!(receipt.schema, REMEDIATION_RECEIPT_V1_SCHEMA,);

    assert_eq!(receipt.status(), RemediationStatus::Verified,);

    assert!(receipt.is_verified());

    assert_eq!(receipt.outcome.changed_files, 1,);

    assert_eq!(receipt.outcome.verification_checks, 1,);

    assert!(receipt.outcome.verification_passed);

    assert_eq!(receipt.effect.before_diagnostics, 1,);

    assert_eq!(receipt.effect.after_diagnostics, 0,);

    assert_eq!(receipt.effect.resolved, 1,);

    assert_eq!(receipt.effect.new, 0,);

    assert_eq!(receipt.effect.introduced_errors, 0,);

    assert!(receipt.changed_diagnostic_state());

    fs::remove_dir_all(root).expect("test directory should clean up");
}

#[test]
fn receipt_does_not_expose_plan_paths_or_edit_contents() {
    let root = temporary_directory("privacy");

    let secret_path = root.join("private-source.txt");

    let plan = FixPlan::new("private remediation")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            secret_path.clone(),
            TextRange::new(0, 11),
            "secret-old",
            "secret-new",
        ));

    let before = DiagnosticReport::new();

    let after = DiagnosticReport::new();

    let applied = FixPlanReport {
        changed_files: vec![secret_path.clone()],
        verification_checks: 0,
        verification_passed: true,
    };

    let receipt = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
        .expect("receipt should be created");

    let json = serde_json::to_string_pretty(&receipt).expect("receipt should serialize");

    assert!(!json.contains(&root.display().to_string(),));

    assert!(!json.contains("private-source.txt",));

    assert!(!json.contains("secret-old",));

    assert!(!json.contains("secret-new",));

    assert!(json.contains("descriptor_digest"));

    assert_eq!(receipt.status(), RemediationStatus::Applied,);
}

#[test]
fn exact_plan_descriptor_has_stable_digest() {
    let path = PathBuf::from("src/value.txt");

    let plan = FixPlan::new("stable remediation")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            path.clone(),
            TextRange::new(0, 3),
            "old",
            "new",
        ));

    let before = DiagnosticReport::new();

    let after = DiagnosticReport::new();

    let applied = FixPlanReport {
        changed_files: vec![path],
        verification_checks: 0,
        verification_passed: true,
    };

    let first = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
        .expect("first receipt should build");

    let second = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
        .expect("second receipt should build");

    assert_eq!(first.plan.descriptor_digest, second.plan.descriptor_digest,);

    assert_eq!(
        first.plan.descriptor_byte_length,
        second.plan.descriptor_byte_length,
    );
}

#[test]
fn receipt_rejects_unverified_apply_report() {
    let plan = FixPlan::new("invalid report")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "value.txt",
            TextRange::new(0, 3),
            "old",
            "new",
        ));

    let report = DiagnosticReport::new();

    let applied = FixPlanReport {
        changed_files: Vec::new(),
        verification_checks: 0,
        verification_passed: false,
    };

    let error = RemediationReceipt::from_successful_apply(&report, &report, &plan, &applied)
        .expect_err("unverified apply report must fail");

    assert!(matches!(
        error,
        RemediationReceiptError::VerificationNotPassed
    ));
}

#[test]
fn receipt_rejects_verification_count_mismatch() {
    let plan = FixPlan::new("mismatched report")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "value.txt",
            TextRange::new(0, 3),
            "old",
            "new",
        ))
        .verify(FileCheck::contains("value.txt", "new"));

    let report = DiagnosticReport::new();

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from("value.txt")],
        verification_checks: 0,
        verification_passed: true,
    };

    let error = RemediationReceipt::from_successful_apply(&report, &report, &plan, &applied)
        .expect_err("mismatched verification count must fail");

    assert!(matches!(
        error,
        RemediationReceiptError::VerificationCountMismatch {
            expected: 1,
            actual: 0,
        }
    ));
}
