use diagprint::{DeltaCounts, DeltaKind, DiagnosticReport, Reporter, Severity};
use diagprint_test::prelude::*;

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

#[test]
fn identical_diagnostics_have_matching_fingerprints_and_digests() {
    let reporter = reporter();

    let first = reporter.warning("unused binding").code("lint::unused");

    let second = reporter.warning("unused binding").code("lint::unused");

    first
        .assert_same_fingerprint_as(&second)
        .assert_same_digest_as(&second);
}

#[test]
fn logical_identity_can_persist_while_content_changes() {
    let reporter = reporter();

    let baseline = reporter
        .warning("unexpected token")
        .code("parse::unexpected")
        .label("src/main.rs", 7, Some(5), Some(1), Some("unexpected here"));

    let candidate = reporter
        .error("unexpected token")
        .code("parse::unexpected")
        .label("src/main.rs", 18, Some(2), Some(1), Some("unexpected here"))
        .help("remove the token");

    baseline
        .assert_same_fingerprint_as(&candidate)
        .assert_different_digest_from(&candidate);
}

#[test]
fn different_messages_produce_different_default_fingerprints() {
    let reporter = reporter();

    let first = reporter.error("expected expression").code("parse::syntax");

    let second = reporter.error("expected identifier").code("parse::syntax");

    first.assert_different_fingerprint_from(&second);
}

#[test]
fn report_digest_is_independent_of_insertion_order() {
    let reporter = reporter();

    let info = reporter
        .info("analysis complete")
        .code("analysis::complete");

    let error = reporter.error("parse failed").code("parse::failed");

    let mut first = DiagnosticReport::new();
    first.push(info.clone()).push(error.clone());

    let mut second = DiagnosticReport::new();
    second.push(error).push(info);

    first.assert_same_digest_as(&second);
}

#[test]
fn semantic_delta_assertions_cover_changed_diagnostics() {
    let reporter = reporter();

    let baseline_diagnostic = reporter
        .warning("unexpected token")
        .code("parse::unexpected");

    let candidate_diagnostic = reporter.error("unexpected token").code("parse::unexpected");

    let mut baseline = DiagnosticReport::new();
    baseline.push(baseline_diagnostic);

    let mut candidate = DiagnosticReport::new();
    candidate.push(candidate_diagnostic);

    let delta = candidate
        .delta_from(&baseline)
        .expect("canonical delta should compute");

    delta
        .assert_has_differences()
        .assert_delta_counts(DeltaCounts {
            changed: 1,
            ..DeltaCounts::default()
        })
        .assert_kind_count(DeltaKind::Changed, 1)
        .assert_introduced_at_or_above(Severity::Error, 1)
        .assert_severity_increases(1)
        .assert_severity_increases_at_or_above(Severity::Error, 1);
}

#[test]
fn identical_reports_produce_an_unchanged_delta() {
    let reporter = reporter();

    let diagnostic = reporter.warning("unused binding").code("lint::unused");

    let mut baseline = DiagnosticReport::new();
    baseline.push(diagnostic.clone());

    let mut candidate = DiagnosticReport::new();
    candidate.push(diagnostic);

    let delta = candidate
        .delta_from(&baseline)
        .expect("canonical delta should compute");

    delta
        .assert_unchanged()
        .assert_delta_counts(DeltaCounts {
            persisting: 1,
            ..DeltaCounts::default()
        })
        .assert_kind_count(DeltaKind::Persisting, 1)
        .assert_severity_increases(0);
}
