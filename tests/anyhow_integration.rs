#![cfg(feature = "anyhow")]

use anyhow::Context as _;
use diagprint::{AnyhowDiagnosticExt, Applicability, Reporter, Severity};
use std::io;

fn nested_error(kind: io::ErrorKind) -> anyhow::Error {
    Err::<(), _>(io::Error::new(kind, "low-level failure"))
        .context("middle context")
        .context("outer context")
        .unwrap_err()
}

#[test]
fn anyhow_outer_context_becomes_message() {
    let reporter = Reporter::builder().build().unwrap();
    let error = nested_error(io::ErrorKind::NotFound);

    let diagnostic = reporter.error_from_anyhow(&error);

    assert_eq!(diagnostic.message, "outer context");
    assert_eq!(diagnostic.severity, Severity::Error);
}

#[test]
fn anyhow_chain_is_preserved_in_order() {
    let reporter = Reporter::builder().build().unwrap();
    let error = nested_error(io::ErrorKind::NotFound);

    let diagnostic = reporter.error_from_anyhow(&error);

    let cause = diagnostic.cause.expect("cause chain");

    assert_eq!(cause.message, "middle context");

    let root = cause.source.expect("root cause");

    assert_eq!(root.message, "low-level failure");
    assert!(root.source.is_none());
}

#[test]
fn anyhow_extension_trait_matches_reporter_adapter() {
    let reporter = Reporter::builder().build().unwrap();
    let error = nested_error(io::ErrorKind::InvalidData);

    let via_reporter = reporter.error_from_anyhow(&error);
    let via_trait = error.to_diagprint(&reporter);

    assert_eq!(via_reporter.message, via_trait.message);

    assert_eq!(
        via_reporter.cause.as_ref().map(|cause| &cause.message),
        via_trait.cause.as_ref().map(|cause| &cause.message)
    );
}

#[test]
fn severity_can_be_overridden() {
    let reporter = Reporter::builder().build().unwrap();
    let error = nested_error(io::ErrorKind::TimedOut);

    let diagnostic = error.to_diagprint_with_severity(&reporter, Severity::Fatal);

    assert_eq!(diagnostic.severity, Severity::Fatal);
}

#[test]
fn io_errors_receive_manual_intelligence() {
    let reporter = Reporter::builder().build().unwrap();
    let error = nested_error(io::ErrorKind::PermissionDenied);

    let diagnostic = reporter.error_from_anyhow(&error);

    assert_eq!(diagnostic.suggestions.len(), 1);

    let suggestion = &diagnostic.suggestions[0];

    assert_eq!(suggestion.applicability, Applicability::Manual);

    assert!(suggestion.title.to_ascii_lowercase().contains("permission"));

    assert_eq!(suggestion.documentation.len(), 1);

    assert!(
        suggestion.documentation[0]
            .url
            .contains("std/io/enum.ErrorKind.html")
    );
}

#[test]
fn unknown_io_kinds_still_receive_safe_generic_guidance() {
    let reporter = Reporter::builder().build().unwrap();

    let error = nested_error(io::ErrorKind::Other);

    let diagnostic = reporter.error_from_anyhow(&error);

    let suggestion = diagnostic
        .suggestions
        .first()
        .expect("generic I/O suggestion");

    assert_eq!(suggestion.applicability, Applicability::Manual);

    assert!(suggestion.title.contains("underlying I/O failure"));
}

#[test]
fn non_io_anyhow_errors_do_not_invent_fixes() {
    let reporter = Reporter::builder().build().unwrap();

    let error = anyhow::anyhow!("parser rejected input").context("configuration parsing failed");

    let diagnostic = reporter.error_from_anyhow(&error);

    assert_eq!(diagnostic.message, "configuration parsing failed");

    assert!(diagnostic.suggestions.is_empty());
}
