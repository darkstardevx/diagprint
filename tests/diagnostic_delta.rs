use diagprint::{
    Applicability, CanonicalizationError, DeltaKind, DiagnosticChange, DiagnosticDelta,
    DiagnosticReport, DiagnosticValue, Edit, IDENTITY_ATTRIBUTE, Reporter, Severity, Suggestion,
    TextRange,
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("delta-test")
        .build()
        .unwrap()
}

fn report(diagnostics: impl IntoIterator<Item = diagprint::Diagnostic>) -> DiagnosticReport {
    diagnostics.into_iter().collect()
}

#[test]
fn empty_reports_are_unchanged() {
    let baseline = DiagnosticReport::new();
    let candidate = DiagnosticReport::new();

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert!(delta.is_unchanged());
    assert_eq!(delta.counts().total_entries(), 0);
    assert_eq!(delta.counts().baseline_total(), 0);
    assert_eq!(delta.counts().candidate_total(), 0);
    assert_eq!(delta.baseline_digest(), delta.candidate_digest());
}

#[test]
fn classifies_new_resolved_persisting_and_changed() {
    let reporter = reporter();

    let persisting_baseline =
        reporter
            .warning("persisting")
            .code("D-PERSIST")
            .source("src/persist.rs", 1, None);

    let persisting_candidate =
        reporter
            .warning("persisting")
            .code("D-PERSIST")
            .source("src/persist.rs", 1, None);

    let changed_baseline =
        reporter
            .warning("changed")
            .code("D-CHANGED")
            .source("src/changed.rs", 10, None);

    let changed_candidate =
        reporter
            .error("changed")
            .code("D-CHANGED")
            .source("src/changed.rs", 99, None);

    let resolved = reporter
        .error("resolved")
        .code("D-RESOLVED")
        .source("src/resolved.rs", 1, None);

    let new = reporter
        .error("new")
        .code("D-NEW")
        .source("src/new.rs", 1, None);

    let baseline = report([persisting_baseline, changed_baseline, resolved]);

    let candidate = report([persisting_candidate, changed_candidate, new]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();
    let counts = delta.counts();

    assert_eq!(counts.new, 1);
    assert_eq!(counts.resolved, 1);
    assert_eq!(counts.persisting, 1);
    assert_eq!(counts.changed, 1);

    assert_eq!(counts.baseline_total(), 3);
    assert_eq!(counts.candidate_total(), 3);
    assert_eq!(counts.differences(), 3);

    assert!(delta.has_differences());

    assert_eq!(delta.iter_kind(DeltaKind::New).count(), 1);
    assert_eq!(delta.iter_kind(DeltaKind::Resolved).count(), 1);
    assert_eq!(delta.iter_kind(DeltaKind::Persisting).count(), 1);
    assert_eq!(delta.iter_kind(DeltaKind::Changed).count(), 1);
}

#[test]
fn severity_and_location_change_are_changed_not_new_and_resolved() {
    let reporter = reporter();

    let baseline = report([reporter
        .warning("same logical diagnostic")
        .code("D100")
        .source("src/main.rs", 10, Some(4))]);

    let candidate = report([reporter
        .error("same logical diagnostic")
        .code("D100")
        .source("src/main.rs", 200, Some(9))]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.counts().changed, 1);
    assert_eq!(delta.counts().new, 0);
    assert_eq!(delta.counts().resolved, 0);

    let changed = delta
        .iter_kind(DeltaKind::Changed)
        .next()
        .expect("changed entry");

    assert_eq!(
        changed.severity_transition(),
        Some((Severity::Warning, Severity::Error))
    );

    assert!(changed.is_severity_increase());
}

#[test]
fn explicit_identity_survives_wording_changes() {
    let reporter = reporter();

    let baseline = report([reporter.warning("old wording").code("D200").attribute(
        IDENTITY_ATTRIBUTE,
        DiagnosticValue::String("parser.rule.17".to_owned()),
    )]);

    let candidate = report([reporter
        .warning("completely different wording")
        .code("D200")
        .attribute(
            IDENTITY_ATTRIBUTE,
            DiagnosticValue::String("parser.rule.17".to_owned()),
        )]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.counts().changed, 1);
    assert_eq!(delta.counts().new, 0);
    assert_eq!(delta.counts().resolved, 0);
}

#[test]
fn derived_identity_treats_wording_change_as_new_and_resolved() {
    let reporter = reporter();

    let baseline = report([reporter.warning("old wording").code("D300")]);
    let candidate = report([reporter.warning("new wording").code("D300")]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.counts().changed, 0);
    assert_eq!(delta.counts().resolved, 1);
    assert_eq!(delta.counts().new, 1);
}

#[test]
fn duplicate_diagnostics_are_preserved_as_a_multiset() {
    let reporter = reporter();

    let baseline = report([
        reporter.warning("duplicate").code("D400"),
        reporter.warning("duplicate").code("D400"),
    ]);

    let candidate = report([reporter.warning("duplicate").code("D400")]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.counts().persisting, 1);
    assert_eq!(delta.counts().resolved, 1);
    assert_eq!(delta.counts().new, 0);
    assert_eq!(delta.counts().changed, 0);

    assert_eq!(delta.counts().baseline_total(), 2);
    assert_eq!(delta.counts().candidate_total(), 1);
}

#[test]
fn exact_matches_are_preferred_before_changed_pairing() {
    let reporter = reporter();

    let baseline = report([
        reporter.warning("bucket").code("D500"),
        reporter.error("bucket").code("D500"),
    ]);

    let candidate = report([
        reporter.error("bucket").code("D500"),
        reporter.fatal("bucket").code("D500"),
    ]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.counts().persisting, 1);
    assert_eq!(delta.counts().changed, 1);
    assert_eq!(delta.counts().new, 0);
    assert_eq!(delta.counts().resolved, 0);

    let persisting = delta
        .iter_kind(DeltaKind::Persisting)
        .next()
        .expect("persisting entry");

    assert_eq!(
        persisting.baseline().map(|diagnostic| diagnostic.severity),
        Some(Severity::Error)
    );

    assert_eq!(
        persisting.candidate().map(|diagnostic| diagnostic.severity),
        Some(Severity::Error)
    );
}

#[test]
fn threshold_introduction_distinguishes_new_failures_from_existing_failures() {
    let reporter = reporter();

    let baseline = report([
        reporter.warning("promoted").code("D600"),
        reporter.error("already failing").code("D601"),
    ]);

    let candidate = report([
        reporter.error("promoted").code("D600"),
        reporter.fatal("already failing").code("D601"),
        reporter.error("brand new").code("D602"),
    ]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.introduced_at_or_above(Severity::Error), 2);
    assert!(delta.has_introduced_at_or_above(Severity::Error));

    assert_eq!(delta.severity_increases(), 2);
    assert_eq!(delta.severity_increases_at_or_above(Severity::Error), 2);
}

#[test]
fn candidate_report_convenience_matches_direct_comparison() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same").code("D700")]);
    let candidate = report([reporter.error("same").code("D700")]);

    let direct = DiagnosticDelta::between(&baseline, &candidate).unwrap();
    let convenience = candidate.delta_from(&baseline).unwrap();

    assert_eq!(direct.counts(), convenience.counts());
    assert_eq!(direct.baseline_digest(), convenience.baseline_digest());
    assert_eq!(direct.candidate_digest(), convenience.candidate_digest());
}

#[test]
fn entry_order_is_independent_of_report_insertion_order() {
    let reporter = reporter();

    let first = reporter.error("alpha").code("A");
    let second = reporter.warning("beta").code("B");
    let third = reporter.info("gamma").code("C");

    let baseline_a = report([first.clone(), second.clone(), third.clone()]);

    let baseline_b = report([third.clone(), first.clone(), second.clone()]);

    let candidate_a = report([
        reporter.error("alpha").code("A"),
        reporter.error("beta").code("B"),
        reporter.info("delta").code("D"),
    ]);

    let candidate_b = report([
        reporter.info("delta").code("D"),
        reporter.error("beta").code("B"),
        reporter.error("alpha").code("A"),
    ]);

    let delta_a = DiagnosticDelta::between(&baseline_a, &candidate_a).unwrap();
    let delta_b = DiagnosticDelta::between(&baseline_b, &candidate_b).unwrap();

    let signature = |delta: &DiagnosticDelta| {
        delta
            .iter()
            .map(|entry| {
                (
                    entry.kind(),
                    entry.fingerprint(),
                    entry.baseline_digest(),
                    entry.candidate_digest(),
                )
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(signature(&delta_a), signature(&delta_b));
}

#[cfg(unix)]
#[test]
fn delta_propagates_non_utf8_canonicalization_failure() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

    let reporter = reporter();

    let invalid_path = PathBuf::from(OsString::from_vec(vec![
        b's', b'r', b'c', b'/', 0xff, b'.', b'r', b's',
    ]));

    let suggestion = Suggestion::new("replace")
        .applicability(Applicability::MachineApplicable)
        .edit(Edit::replace(invalid_path, TextRange::new(0, 1), "a", "b"));

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter
        .error("invalid remediation path")
        .suggestion(suggestion)]);

    let error = DiagnosticDelta::between(&baseline, &candidate).unwrap_err();

    assert!(matches!(error, CanonicalizationError::NonUtf8Path { .. }));
}

#[test]
fn changed_entries_have_distinct_digests() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same").code("D800").help("old help")]);

    let candidate = report([reporter.warning("same").code("D800").help("new help")]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    let entry = delta
        .iter_kind(DeltaKind::Changed)
        .next()
        .expect("changed entry");

    let DiagnosticChange::Changed {
        baseline_digest,
        candidate_digest,
        ..
    } = entry
    else {
        panic!("expected changed diagnostic");
    };

    assert_ne!(baseline_digest, candidate_digest);
}

#[test]
fn threshold_introduction_is_independent_of_duplicate_pairing() {
    let reporter = reporter();

    let baseline = report([
        reporter.warning("duplicate bucket").code("D900"),
        reporter.error("duplicate bucket").code("D900"),
    ]);

    let candidate = report([
        reporter.error("duplicate bucket").code("D900"),
        reporter.fatal("duplicate bucket").code("D900"),
    ]);

    let delta = DiagnosticDelta::between(&baseline, &candidate).unwrap();

    assert_eq!(delta.introduced_at_or_above(Severity::Error), 1);

    assert_eq!(delta.introduced_at_or_above(Severity::Fatal), 1);
}
