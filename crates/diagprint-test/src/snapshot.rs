use diagprint::{Diagnostic, DiagnosticReport};
use serde_json::Value;

/// Runtime-specific diagnostic fields deliberately excluded from stable
/// snapshots.
///
/// These fields identify one particular diagnostic occurrence or process rather
/// than the semantic diagnostic payload.
pub const SNAPSHOT_IGNORED_FIELDS: [&str; 5] =
    ["report_id", "session_id", "timestamp", "pid", "hostname"];

/// Produces stable, pretty-printed JSON for one diagnostic.
///
/// Runtime identity and process metadata are removed while structured
/// diagnostic content remains intact.
#[must_use]
pub fn diagnostic_snapshot(diagnostic: &Diagnostic) -> String {
    pretty_json(&stable_diagnostic_value(diagnostic))
}

/// Produces stable, pretty-printed JSON for a diagnostic report.
///
/// Diagnostics are deterministically sorted before serialization so report
/// insertion order does not affect the snapshot.
#[must_use]
pub fn report_snapshot(report: &DiagnosticReport) -> String {
    let mut report = report.clone();
    report.sort_deterministic();

    let diagnostics = report
        .iter()
        .map(stable_diagnostic_value)
        .collect::<Vec<_>>();

    pretty_json(&Value::Array(diagnostics))
}

/// Compares one diagnostic against an expected JSON snapshot.
///
/// JSON formatting and object-key ordering in `expected` do not matter.
pub fn assert_diagnostic_matches_snapshot(diagnostic: &Diagnostic, expected: impl AsRef<str>) {
    assert_snapshot(
        stable_diagnostic_value(diagnostic),
        expected.as_ref(),
        "diagnostic",
    );
}

/// Compares one report against an expected JSON snapshot.
///
/// Report diagnostics are deterministically sorted first.
pub fn assert_report_matches_snapshot(report: &DiagnosticReport, expected: impl AsRef<str>) {
    let actual = serde_json::from_str::<Value>(&report_snapshot(report)).unwrap_or_else(|error| {
        panic!("failed to parse generated diagprint report snapshot: {error}")
    });

    assert_snapshot(actual, expected.as_ref(), "diagnostic report");
}

fn stable_diagnostic_value(diagnostic: &Diagnostic) -> Value {
    let mut value = serde_json::to_value(diagnostic).unwrap_or_else(|error| {
        panic!("failed to serialize diagprint diagnostic for snapshot: {error}")
    });

    remove_runtime_metadata(&mut value);

    value
}

fn remove_runtime_metadata(value: &mut Value) {
    let Value::Object(object) = value else {
        panic!("diagprint diagnostic did not serialize to a JSON object");
    };

    for field in SNAPSHOT_IGNORED_FIELDS {
        object.remove(field);
    }
}

fn assert_snapshot(actual: Value, expected: &str, subject: &str) {
    let expected = serde_json::from_str::<Value>(expected)
        .unwrap_or_else(|error| panic!("invalid expected {subject} JSON snapshot: {error}"));

    if actual == expected {
        return;
    }

    panic!(
        "{subject} snapshot mismatch\n\nexpected:\n{}\n\nactual:\n{}",
        pretty_json(&expected),
        pretty_json(&actual),
    );
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| panic!("failed to format diagprint JSON snapshot: {error}"))
}

/// Asserts that a diagnostic matches an inline JSON snapshot.
///
/// Runtime-specific fields such as UUIDs, timestamps, PID, and hostname are
/// ignored automatically.
#[macro_export]
macro_rules! assert_diagnostic_snapshot {
    ($diagnostic:expr, $expected:expr $(,)?) => {{
        $crate::assert_diagnostic_matches_snapshot(&($diagnostic), $expected);
    }};
}

/// Asserts that a diagnostic report matches an inline JSON snapshot.
///
/// Diagnostics are sorted deterministically and runtime-specific fields are
/// ignored automatically.
#[macro_export]
macro_rules! assert_report_snapshot {
    ($report:expr, $expected:expr $(,)?) => {{
        $crate::assert_report_matches_snapshot(&($report), $expected);
    }};
}

/// Internal helper used by file-backed snapshot assertion macros.
///
/// Snapshot updates are enabled with:
///
/// `DIAGPRINT_UPDATE_SNAPSHOTS=1`
#[doc(hidden)]
#[macro_export]
macro_rules! __diagprint_snapshot_update_enabled {
    () => {{
        match ::std::env::var("DIAGPRINT_UPDATE_SNAPSHOTS") {
            Ok(value) => {
                let value = value.to_ascii_lowercase();

                match value.as_str() {
                    "1" | "true" | "yes" | "on" | "always" => true,
                    "" | "0" | "false" | "no" | "off" | "never" => false,
                    other => {
                        panic!(
                            "invalid DIAGPRINT_UPDATE_SNAPSHOTS value {other:?}; \
                             expected one of: 1, true, yes, on, always, \
                             0, false, no, off, never"
                        );
                    }
                }
            }

            Err(::std::env::VarError::NotPresent) => false,

            Err(::std::env::VarError::NotUnicode(_)) => {
                panic!("DIAGPRINT_UPDATE_SNAPSHOTS contains non-Unicode data");
            }
        }
    }};
}

/// Asserts that a diagnostic matches a JSON snapshot file.
///
/// The path is resolved relative to the calling crate's `CARGO_MANIFEST_DIR`.
///
/// Set `DIAGPRINT_UPDATE_SNAPSHOTS=1` to create or replace the snapshot file
/// with the current deterministic representation.
#[macro_export]
macro_rules! assert_diagnostic_file_snapshot {
    ($diagnostic:expr, $path:literal $(,)?) => {{
        let __diagnostic = &($diagnostic);
        let __path = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join($path);

        if $crate::__diagprint_snapshot_update_enabled!() {
            let __snapshot = $crate::diagnostic_snapshot(__diagnostic);

            if let Some(__parent) = __path.parent() {
                ::std::fs::create_dir_all(__parent).unwrap_or_else(|error| {
                    panic!(
                        "failed to create diagprint snapshot directory {}: {error}",
                        __parent.display(),
                    )
                });
            }

            ::std::fs::write(&__path, format!("{__snapshot}\n")).unwrap_or_else(|error| {
                panic!(
                    "failed to write diagprint diagnostic snapshot {}: {error}",
                    __path.display(),
                )
            });
        } else {
            let __expected = ::std::fs::read_to_string(&__path).unwrap_or_else(|error| {
                panic!(
                    "failed to read diagprint diagnostic snapshot {}: {error}\n\
                         set DIAGPRINT_UPDATE_SNAPSHOTS=1 to create or update it",
                    __path.display(),
                )
            });

            $crate::assert_diagnostic_matches_snapshot(__diagnostic, &__expected);
        }
    }};
}

/// Asserts that a diagnostic report matches a JSON snapshot file.
///
/// The path is resolved relative to the calling crate's `CARGO_MANIFEST_DIR`.
///
/// Report diagnostics are sorted deterministically before serialization.
///
/// Set `DIAGPRINT_UPDATE_SNAPSHOTS=1` to create or replace the snapshot file
/// with the current deterministic representation.
#[macro_export]
macro_rules! assert_report_file_snapshot {
    ($report:expr, $path:literal $(,)?) => {{
        let __report = &($report);
        let __path = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join($path);

        if $crate::__diagprint_snapshot_update_enabled!() {
            let __snapshot = $crate::report_snapshot(__report);

            if let Some(__parent) = __path.parent() {
                ::std::fs::create_dir_all(__parent).unwrap_or_else(|error| {
                    panic!(
                        "failed to create diagprint snapshot directory {}: {error}",
                        __parent.display(),
                    )
                });
            }

            ::std::fs::write(&__path, format!("{__snapshot}\n")).unwrap_or_else(|error| {
                panic!(
                    "failed to write diagprint report snapshot {}: {error}",
                    __path.display(),
                )
            });
        } else {
            let __expected = ::std::fs::read_to_string(&__path).unwrap_or_else(|error| {
                panic!(
                    "failed to read diagprint report snapshot {}: {error}\n\
                         set DIAGPRINT_UPDATE_SNAPSHOTS=1 to create or update it",
                    __path.display(),
                )
            });

            $crate::assert_report_matches_snapshot(__report, &__expected);
        }
    }};
}
