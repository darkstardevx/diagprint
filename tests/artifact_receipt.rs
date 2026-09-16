use diagprint::{
    ArtifactEncoding, ArtifactVerificationError, DeltaPolicy, DiagnosticReport, ExportAttributes,
    ExportPath, ExportPolicy, ExportText, ExportUrl, RECEIPT_V1_SCHEMA, Reporter,
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("artifact-receipt-test")
        .build()
        .unwrap()
}

fn report(diagnostics: impl IntoIterator<Item = diagprint::Diagnostic>) -> DiagnosticReport {
    diagnostics.into_iter().collect()
}

#[test]
fn exported_artifact_verifies_against_its_receipt() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same").code("R100")]);

    let candidate = report([reporter.error("same").code("R100")]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let exported = delta
        .export_evaluated_json(&DeltaPolicy::ci(), &ExportPolicy::default())
        .unwrap();

    assert_eq!(exported.receipt().schema, RECEIPT_V1_SCHEMA);

    assert_eq!(exported.receipt().encoding, ArtifactEncoding::CompactJson);

    assert_eq!(exported.receipt().byte_length, exported.bytes().len());

    exported.verify().unwrap();

    assert_eq!(exported.receipt().baseline_report, delta.baseline_digest());

    assert_eq!(
        exported.receipt().candidate_report,
        delta.candidate_digest()
    );
}

#[test]
fn compact_and_pretty_exports_have_same_semantics_but_different_artifact_identity() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same").code("R200")]);

    let candidate = report([reporter.error("same").code("R200")]);

    let delta = candidate.delta_from(&baseline).unwrap();
    let policy = ExportPolicy::default();

    let compact = delta.export_json(&policy).unwrap();
    let pretty = delta.export_json_pretty(&policy).unwrap();

    assert_eq!(
        compact.receipt().baseline_report,
        pretty.receipt().baseline_report
    );

    assert_eq!(
        compact.receipt().candidate_report,
        pretty.receipt().candidate_report
    );

    assert_ne!(
        compact.receipt().artifact_digest,
        pretty.receipt().artifact_digest
    );

    assert_ne!(compact.bytes(), pretty.bytes());
}

#[test]
fn tampered_bytes_fail_receipt_verification() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter.error("new").code("R300")]);

    let delta = candidate.delta_from(&baseline).unwrap();
    let exported = delta.export_json(&ExportPolicy::default()).unwrap();

    let mut tampered = exported.bytes().to_vec();

    let index = tampered
        .iter()
        .position(|byte| *byte == b'n')
        .expect("test artifact should contain an n byte");

    tampered[index] = b'N';

    let error = exported.receipt().verify_bytes(&tampered).unwrap_err();

    assert!(matches!(error, ArtifactVerificationError::Digest { .. }));
}

#[test]
fn truncated_bytes_fail_length_verification() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter.warning("new").code("R400")]);

    let delta = candidate.delta_from(&baseline).unwrap();
    let exported = delta.export_json(&ExportPolicy::default()).unwrap();

    let truncated = &exported.bytes()[..exported.bytes().len() - 1];

    let error = exported.receipt().verify_bytes(truncated).unwrap_err();

    assert!(matches!(error, ArtifactVerificationError::Length { .. }));
}

#[test]
fn receipt_records_export_boundary_without_repository_root() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter.error("private").code("R500").source(
        "/home/alice/private/project/src/main.rs",
        7,
        None,
    )]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let policy = ExportPolicy::new()
        .with_text(ExportText::Redact)
        .with_paths(ExportPath::RepositoryRelative(
            "/home/alice/private/project".into(),
        ))
        .with_attributes(ExportAttributes::RedactSensitive)
        .with_urls(ExportUrl::Omit)
        .with_application(false);

    let exported = delta.export_json(&policy).unwrap();

    let descriptor = exported.receipt().export_policy;

    assert_eq!(descriptor.text, "redact");
    assert_eq!(descriptor.paths, "repository_relative");
    assert_eq!(descriptor.attributes, "redact_sensitive");
    assert_eq!(descriptor.urls, "omit");
    assert!(!descriptor.include_application);

    let receipt_json = serde_json::to_string(exported.receipt()).unwrap();

    assert!(!receipt_json.contains("/home/alice/private/project"));
}

#[test]
fn evaluated_receipt_records_ci_result() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter.error("new failure").code("R600")]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let exported = delta
        .export_evaluated_json(&DeltaPolicy::ci(), &ExportPolicy::default())
        .unwrap();

    let evaluation = exported
        .receipt()
        .evaluation
        .expect("evaluated export should retain CI result");

    assert_eq!(evaluation.status, "failure");
    assert_eq!(evaluation.exit_code, 1);
}

#[test]
fn repeated_export_of_same_delta_is_byte_identical() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same").code("R700")]);

    let candidate = report([reporter.error("same").code("R700")]);

    let delta = candidate.delta_from(&baseline).unwrap();
    let policy = ExportPolicy::default();

    let first = delta.export_json(&policy).unwrap();
    let second = delta.export_json(&policy).unwrap();

    assert_eq!(first.bytes(), second.bytes());

    assert_eq!(
        first.receipt().artifact_digest,
        second.receipt().artifact_digest
    );
}
