use diagprint::{
    CANONICAL_V1_NAMESPACE, CanonicalizationVersion, Cause, Diagnostic, DiagnosticAttribute,
    DiagnosticReport, DiagnosticValue, DocumentationLink, FingerprintSource, IDENTITY_ATTRIBUTE,
    Reporter, Severity, Suggestion,
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("compiler")
        .build()
        .expect("test reporter should build")
}

fn regression_diagnostic() -> Diagnostic {
    reporter()
        .error("example failure")
        .code("E0001")
        .attribute("z", 7_u64)
        .attribute("a", "value")
        .label("src/lib.rs", 10, Some(5), Some(3), Some("here"))
        .secondary_label("src/config.rs", 2, None, None, None::<String>)
        .note("note one")
        .help("try this")
        .cause_chain(Cause::new("outer").caused_by(Cause::new("inner")))
        .suggestion(
            Suggestion::new("read docs").documentation(
                DocumentationLink::new("Guide", "https://example.com").language("rust"),
            ),
        )
}

#[test]
fn canonical_v1_namespace_is_stable() {
    assert_eq!(CanonicalizationVersion::V1.as_str(), CANONICAL_V1_NAMESPACE);
    assert_eq!(CANONICAL_V1_NAMESPACE, "diagprint.canonical/v1");
}

#[test]
fn canonical_v1_regression_vectors_are_stable() {
    let diagnostic = regression_diagnostic();

    let fingerprint = diagnostic.fingerprint();
    let digest = diagnostic.digest().expect("diagnostic should canonicalize");
    let report = DiagnosticReport::from_diagnostic(diagnostic);
    let report_digest = report.digest().expect("report should canonicalize");

    assert_eq!(
        fingerprint.to_string(),
        "sha256:8c732c2415db09d262134744f6fc0a56d15aab46976cbfcd25ea3bde94a25eb9"
    );

    assert_eq!(
        digest.to_string(),
        "sha256:6843707ed2003a2ebd032c8f7d8a1a12711c7787716d5f1a4bc2f850849c7cd4"
    );

    assert_eq!(
        report_digest.to_string(),
        "sha256:3be3e2391c5fabca69989980d6d736644d52058608dd0d22d75d07e4b89cec37"
    );

    assert_eq!(fingerprint.version(), CanonicalizationVersion::V1);
    assert_eq!(digest.version(), CanonicalizationVersion::V1);
    assert_eq!(report_digest.version(), CanonicalizationVersion::V1);
}

#[test]
fn occurrence_metadata_does_not_change_identity() {
    let left = reporter().error("same failure").code("DP1001");
    let right = reporter().error("same failure").code("DP1001");

    assert_ne!(left.report_id, right.report_id);
    assert_ne!(left.session_id, right.session_id);

    assert_eq!(left.fingerprint(), right.fingerprint());
    assert_eq!(
        left.digest().expect("left should canonicalize"),
        right.digest().expect("right should canonicalize")
    );
}

#[test]
fn severity_and_exact_location_change_digest_but_not_default_fingerprint() {
    let left = reporter().warning("unused binding").code("DP2001").label(
        "src/lib.rs",
        10,
        Some(4),
        Some(3),
        Some("unused"),
    );

    let mut right = left.clone();
    right.severity = Severity::Error;
    right.labels[0].location.line = 42;
    right.labels[0].location.column = Some(12);

    assert_eq!(left.fingerprint(), right.fingerprint());
    assert_ne!(
        left.digest().expect("left should canonicalize"),
        right.digest().expect("right should canonicalize")
    );
}

#[test]
fn attribute_insertion_order_does_not_change_digest() {
    let left = reporter()
        .error("failure")
        .attribute("beta", 2_u64)
        .attribute("alpha", "one");

    let right = reporter()
        .error("failure")
        .attribute("alpha", "one")
        .attribute("beta", 2_u64);

    assert_eq!(
        left.digest().expect("left should canonicalize"),
        right.digest().expect("right should canonicalize")
    );
}

#[test]
fn explicit_identity_survives_wording_and_location_changes() {
    let left = reporter()
        .warning("old wording")
        .code("DP3001")
        .attribute(IDENTITY_ATTRIBUTE, "parser.missing-semicolon")
        .label("src/old.rs", 5, Some(2), Some(1), Some("old label"));

    let right = reporter()
        .error("new wording")
        .code("DP3001")
        .attribute(IDENTITY_ATTRIBUTE, "parser.missing-semicolon")
        .label("src/new.rs", 99, Some(8), Some(4), Some("new label"));

    assert_eq!(left.fingerprint(), right.fingerprint());
    assert_ne!(
        left.digest().expect("left should canonicalize"),
        right.digest().expect("right should canonicalize")
    );
}

#[test]
fn fingerprint_source_policy_can_use_full_source_name() {
    let left = reporter().error("failure").code("DP4001").label(
        "crate_a/src/lib.rs",
        1,
        None,
        None,
        None::<String>,
    );

    let right = reporter().error("failure").code("DP4001").label(
        "crate_b/src/lib.rs",
        1,
        None,
        None,
        None::<String>,
    );

    assert_eq!(left.fingerprint(), right.fingerprint());

    let policy = diagprint::FingerprintPolicy::new().with_source(FingerprintSource::Full);

    assert_ne!(
        left.fingerprint_with_policy(&policy),
        right.fingerprint_with_policy(&policy)
    );
}

#[test]
fn report_digest_is_independent_of_insertion_order() {
    let first = reporter().error("first").code("DP5001");
    let second = reporter().warning("second").code("DP5002");

    let mut left = DiagnosticReport::new();
    left.push(first.clone()).push(second.clone());

    let mut right = DiagnosticReport::new();
    right.push(second).push(first);

    assert_eq!(
        left.digest().expect("left report should canonicalize"),
        right.digest().expect("right report should canonicalize")
    );
}

#[test]
fn report_digest_preserves_duplicate_diagnostics() {
    let diagnostic = reporter().error("duplicate").code("DP6001");

    let one = DiagnosticReport::from_diagnostic(diagnostic.clone());

    let mut two = DiagnosticReport::new();
    two.push(diagnostic.clone()).push(diagnostic);

    assert_ne!(
        one.digest().expect("one should canonicalize"),
        two.digest().expect("two should canonicalize")
    );
}

#[test]
fn canonical_floats_normalize_negative_zero_and_nan_payloads() {
    let positive_zero = reporter()
        .error("float")
        .attributes([DiagnosticAttribute::new("value", DiagnosticValue::F64(0.0))]);

    let negative_zero = reporter()
        .error("float")
        .attributes([DiagnosticAttribute::new(
            "value",
            DiagnosticValue::F64(-0.0),
        )]);

    assert_eq!(
        positive_zero
            .digest()
            .expect("positive zero should canonicalize"),
        negative_zero
            .digest()
            .expect("negative zero should canonicalize")
    );

    let first_nan = reporter()
        .error("float")
        .attributes([DiagnosticAttribute::new(
            "value",
            DiagnosticValue::F64(f64::from_bits(0x7ff8_0000_0000_0001)),
        )]);

    let second_nan = reporter()
        .error("float")
        .attributes([DiagnosticAttribute::new(
            "value",
            DiagnosticValue::F64(f64::from_bits(0x7fff_ffff_ffff_ffff)),
        )]);

    assert_eq!(
        first_nan.digest().expect("first NaN should canonicalize"),
        second_nan.digest().expect("second NaN should canonicalize")
    );
}

#[cfg(unix)]
#[test]
fn canonical_v1_rejects_non_utf8_remediation_paths() {
    use diagprint::{CanonicalizationError, Edit};
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

    let path = PathBuf::from(OsString::from_vec(vec![0xff]));

    let diagnostic = reporter()
        .error("bad path")
        .suggestion(Suggestion::new("edit").edit(Edit::insert(path, 0, "text")));

    assert!(matches!(
        diagnostic.digest(),
        Err(CanonicalizationError::NonUtf8Path { .. })
    ));
}
