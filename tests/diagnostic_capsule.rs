use diagprint::{
    Applicability, CapsuleProvenance, DIAGNOSTIC_CAPSULE_V1_SCHEMA, DiagnosticCapsule,
    DiagnosticCapsuleError, DiagnosticCapsulePolicy, DiagnosticReport, Edit, FixPlan,
    FixPlanReport, RemediationReceipt, Reporter, SourceCache, TextRange,
    render::{AuditTranscriptRenderer, CompilerTextRenderer, MarkdownRenderer},
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagnostic-capsule-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

fn sample_report() -> DiagnosticReport {
    let reporter = reporter();

    let mut report = DiagnosticReport::new();

    report
        .push(reporter.warning("unused configuration").code("W100"))
        .push(reporter.error("invalid configuration").code("E100"));

    report
}

fn temporary_destination(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-capsule-{name}-{}-{nonce}.diagpack",
        std::process::id(),
    ))
}

#[test]
fn capsule_manifest_tracks_exact_entries() {
    let report = sample_report();

    let mut capsule = DiagnosticCapsule::new(&report).expect("capsule should build");

    let markdown = MarkdownRenderer
        .render_report_artifact(&report)
        .expect("Markdown should render");

    let compiler = CompilerTextRenderer
        .render_report_artifact(&report)
        .expect("compiler text should render");

    let audit = AuditTranscriptRenderer
        .render_report_artifact(&report)
        .expect("audit transcript should render");

    capsule
        .add_rendered(&markdown)
        .expect("Markdown should be added")
        .add_rendered(&compiler)
        .expect("compiler text should be added")
        .add_rendered(&audit)
        .expect("audit transcript should be added");

    let manifest = capsule.manifest();

    assert_eq!(manifest.schema, DIAGNOSTIC_CAPSULE_V1_SCHEMA,);

    assert_eq!(
        manifest.report_digest,
        report
            .digest()
            .expect("report digest should build",)
            .qualified(),
    );

    // report.json plus three rendered artifacts and three receipts.
    assert_eq!(manifest.entries.len(), 7,);

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "reports/report.json" },)
    );

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "reports/report.md" },)
    );

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "reports/compiler.txt" },)
    );

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "reports/report.audit" },)
    );
}

#[test]
fn capsule_rejects_rendered_artifact_from_another_report() {
    let report = sample_report();

    let other =
        DiagnosticReport::from_diagnostic(reporter().error("different diagnostic").code("E999"));

    let artifact = MarkdownRenderer
        .render_report_artifact(&other)
        .expect("other artifact should render");

    let mut capsule = DiagnosticCapsule::new(&report).expect("capsule should build");

    let error = capsule
        .add_rendered(&artifact)
        .expect_err("unrelated report must fail");

    assert!(matches!(
        error,
        DiagnosticCapsuleError::ReportMismatch { .. }
    ));
}

#[test]
fn capsule_source_export_requires_explicit_policy() {
    let report = sample_report();

    let cache = SourceCache::new();

    cache.insert("../../virtual/secret.rs", "fn example() {}\n");

    let snapshot = cache.snapshot();

    let mut default_capsule = DiagnosticCapsule::new(&report).expect("capsule should build");

    let error = default_capsule
        .add_sources(&snapshot)
        .expect_err("source export should be denied");

    assert!(matches!(error, DiagnosticCapsuleError::PolicyDenied { .. }));

    let policy = DiagnosticCapsulePolicy::new().with_sources(true);

    let mut allowed =
        DiagnosticCapsule::with_policies(&report, &diagprint::ExportPolicy::default(), policy)
            .expect("source-enabled capsule should build");

    allowed
        .add_sources(&snapshot)
        .expect("source export should succeed");

    let manifest = allowed.manifest();

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "sources/index.json" },)
    );

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "sources/000001.txt" },)
    );

    assert!(
        !manifest
            .entries
            .iter()
            .any(|entry| { entry.path.contains("..",) },)
    );
}

#[test]
fn capsule_binds_remediation_plan_to_receipt() {
    let before =
        DiagnosticReport::from_diagnostic(reporter().error("configuration invalid").code("E400"));

    let after = DiagnosticReport::new();

    let plan = FixPlan::new("repair configuration")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(
            "config.txt",
            TextRange::new(0, 3),
            "old",
            "new",
        ));

    let applied = FixPlanReport {
        changed_files: vec![PathBuf::from("config.txt")],

        verification_checks: 0,

        verification_passed: true,
    };

    let receipt = RemediationReceipt::from_successful_apply(&before, &after, &plan, &applied)
        .expect("receipt should build");

    let policy = DiagnosticCapsulePolicy::new().with_remediation_plan(true);

    let mut capsule =
        DiagnosticCapsule::with_policies(&after, &diagprint::ExportPolicy::default(), policy)
            .expect("capsule should build");

    capsule
        .add_remediation(&receipt, Some(&plan))
        .expect("matching remediation should be added");

    let manifest = capsule.manifest();

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "receipts/remediation.json" },)
    );

    assert!(
        manifest
            .entries
            .iter()
            .any(|entry| { entry.path == "remediation/plan.json" },)
    );
}

#[test]
fn persisted_capsule_verifies_and_tampering_is_detected() {
    let report = sample_report();

    let mut capsule = DiagnosticCapsule::new(&report).expect("capsule should build");

    let audit = AuditTranscriptRenderer
        .render_report_artifact(&report)
        .expect("audit should render");

    capsule
        .add_rendered(&audit)
        .expect("audit should be added")
        .add_provenance(
            &CapsuleProvenance::new()
                .attribute("scan_profile", "static")
                .attribute("fixture", "diagnostic_capsule"),
        )
        .expect("provenance should be added");

    let destination = temporary_destination("verify");

    let persisted = capsule
        .write_to(&destination)
        .expect("capsule should persist");

    assert_eq!(persisted.directory(), destination.as_path(),);

    assert!(persisted.manifest_path().is_file());

    let verification =
        DiagnosticCapsule::verify_directory(&destination).expect("capsule should verify");

    assert_eq!(verification.manifest_digest, persisted.manifest_digest(),);

    let audit_path = destination.join("reports/report.audit");

    fs::write(&audit_path, "tampered\n").expect("artifact should be tampered");

    let error =
        DiagnosticCapsule::verify_directory(&destination).expect_err("tampered capsule must fail");

    assert!(matches!(
        error,
        DiagnosticCapsuleError::EntryLength { .. } | DiagnosticCapsuleError::EntryDigest { .. }
    ));

    fs::remove_dir_all(destination).expect("capsule should clean up");
}

#[test]
fn capsule_persistence_is_create_only() {
    let report = sample_report();

    let capsule = DiagnosticCapsule::new(&report).expect("capsule should build");

    let destination = temporary_destination("create-only");

    capsule
        .write_to(&destination)
        .expect("first write should succeed");

    let error = capsule
        .write_to(&destination)
        .expect_err("second write must fail");

    assert!(matches!(
        error,
        DiagnosticCapsuleError::DestinationExists { .. }
    ));

    fs::remove_dir_all(destination).expect("capsule should clean up");
}
