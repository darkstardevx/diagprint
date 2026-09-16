//! Testing helpers for `diagprint`.
//!
//! `diagprint-test` provides focused assertions and deterministic snapshot
//! helpers over structured diagnostics and diagnostic reports without depending
//! on rendered terminal output.
//!
//! The helpers intentionally inspect diagprint's public structured model rather
//! than parsing presentation formats.

#![forbid(unsafe_code)]

mod snapshot;

use diagprint::{
    Applicability, Diagnostic, DiagnosticReport, DiagnosticValue, LabelKind, ReportStatus,
    Severity, SeverityCounts,
};

pub use snapshot::{
    SNAPSHOT_IGNORED_FIELDS, assert_diagnostic_matches_snapshot, assert_report_matches_snapshot,
    diagnostic_snapshot, report_snapshot,
};

/// Assertion helpers for one [`struct@Diagnostic`].
pub trait DiagnosticAssertions {
    /// Asserts that the diagnostic has exactly `expected` severity.
    fn assert_severity(&self, expected: Severity) -> &Self;

    /// Asserts that the diagnostic has exactly `expected` code.
    fn assert_code(&self, expected: &str) -> &Self;

    /// Asserts that the diagnostic has no diagnostic code.
    fn assert_no_code(&self) -> &Self;

    /// Asserts that the diagnostic message contains `expected`.
    fn assert_message_contains(&self, expected: &str) -> &Self;

    /// Asserts that help text exists and contains `expected`.
    fn assert_help_contains(&self, expected: &str) -> &Self;

    /// Asserts that at least one note contains `expected`.
    fn assert_note_contains(&self, expected: &str) -> &Self;

    /// Asserts that a structured attribute has the expected value.
    fn assert_attribute<V>(&self, name: &str, expected: V) -> &Self
    where
        V: Into<DiagnosticValue>;

    /// Asserts that a label with the requested kind, file, and line exists.
    fn assert_label(&self, kind: LabelKind, file: &str, line: u32) -> &Self;

    /// Asserts that a primary label exists at `file:line`.
    fn assert_primary_label(&self, file: &str, line: u32) -> &Self {
        self.assert_label(LabelKind::Primary, file, line)
    }

    /// Asserts that a secondary label exists at `file:line`.
    fn assert_secondary_label(&self, file: &str, line: u32) -> &Self {
        self.assert_label(LabelKind::Secondary, file, line)
    }

    /// Asserts that a suggestion with exactly `title` exists.
    fn assert_suggestion(&self, title: &str) -> &Self;

    /// Asserts that a suggestion has the expected applicability.
    fn assert_suggestion_applicability(&self, title: &str, expected: Applicability) -> &Self;
}

impl DiagnosticAssertions for Diagnostic {
    fn assert_severity(&self, expected: Severity) -> &Self {
        assert_eq!(
            self.severity, expected,
            "diagnostic severity mismatch: expected {expected}, got {}",
            self.severity,
        );

        self
    }

    fn assert_code(&self, expected: &str) -> &Self {
        assert_eq!(
            self.code.as_deref(),
            Some(expected),
            "diagnostic code mismatch: expected {expected:?}, got {:?}",
            self.code,
        );

        self
    }

    fn assert_no_code(&self) -> &Self {
        assert!(
            self.code.is_none(),
            "expected diagnostic to have no code, got {:?}",
            self.code,
        );

        self
    }

    fn assert_message_contains(&self, expected: &str) -> &Self {
        assert!(
            self.message.contains(expected),
            "diagnostic message did not contain {expected:?}: {:?}",
            self.message,
        );

        self
    }

    fn assert_help_contains(&self, expected: &str) -> &Self {
        let help = self.help.as_deref().unwrap_or_else(|| {
            panic!("expected diagnostic help containing {expected:?}, but no help was present")
        });

        assert!(
            help.contains(expected),
            "diagnostic help did not contain {expected:?}: {help:?}",
        );

        self
    }

    fn assert_note_contains(&self, expected: &str) -> &Self {
        assert!(
            self.notes.iter().any(|note| note.contains(expected)),
            "no diagnostic note contained {expected:?}; notes: {:?}",
            self.notes,
        );

        self
    }

    fn assert_attribute<V>(&self, name: &str, expected: V) -> &Self
    where
        V: Into<DiagnosticValue>,
    {
        let expected = expected.into();

        let actual = self
            .attributes
            .iter()
            .find(|attribute| attribute.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "expected diagnostic attribute {name:?}; available attributes: {:?}",
                    self.attributes
                        .iter()
                        .map(|attribute| attribute.name.as_str())
                        .collect::<Vec<_>>(),
                )
            });

        assert_eq!(
            actual.value, expected,
            "diagnostic attribute {name:?} mismatch",
        );

        self
    }

    fn assert_label(&self, kind: LabelKind, file: &str, line: u32) -> &Self {
        assert!(
            self.labels.iter().any(|label| {
                label.kind == kind && label.location.file == file && label.location.line == line
            }),
            "expected {kind:?} label at {file}:{line}; labels: {:?}",
            self.labels,
        );

        self
    }

    fn assert_suggestion(&self, title: &str) -> &Self {
        assert!(
            self.suggestions
                .iter()
                .any(|suggestion| suggestion.title == title),
            "expected suggestion {title:?}; suggestions: {:?}",
            self.suggestions
                .iter()
                .map(|suggestion| suggestion.title.as_str())
                .collect::<Vec<_>>(),
        );

        self
    }

    fn assert_suggestion_applicability(&self, title: &str, expected: Applicability) -> &Self {
        let suggestion = self
            .suggestions
            .iter()
            .find(|suggestion| suggestion.title == title)
            .unwrap_or_else(|| {
                panic!(
                    "expected suggestion {title:?}; suggestions: {:?}",
                    self.suggestions
                        .iter()
                        .map(|suggestion| suggestion.title.as_str())
                        .collect::<Vec<_>>(),
                )
            });

        assert_eq!(
            suggestion.applicability, expected,
            "suggestion {title:?} applicability mismatch",
        );

        self
    }
}

/// Assertion helpers for [`DiagnosticReport`].
pub trait DiagnosticReportAssertions {
    /// Asserts the exact number of diagnostics.
    fn assert_len(&self, expected: usize) -> &Self;

    /// Asserts that the report is empty.
    fn assert_empty(&self) -> &Self;

    /// Asserts that the report is not empty.
    fn assert_not_empty(&self) -> &Self;

    /// Asserts the report's default status.
    fn assert_status(&self, expected: ReportStatus) -> &Self;

    /// Asserts exact severity counts.
    fn assert_counts(&self, expected: SeverityCounts) -> &Self;

    /// Asserts that at least one diagnostic has `severity`.
    fn assert_contains_severity(&self, severity: Severity) -> &Self;

    /// Asserts that at least one diagnostic has code `expected`.
    fn assert_contains_code(&self, expected: &str) -> &Self;

    /// Asserts the number of diagnostics at or above `minimum`.
    fn assert_count_at_or_above(&self, minimum: Severity, expected: usize) -> &Self;

    /// Asserts that no diagnostics meet or exceed `minimum`.
    fn assert_none_at_or_above(&self, minimum: Severity) -> &Self;
}

impl DiagnosticReportAssertions for DiagnosticReport {
    fn assert_len(&self, expected: usize) -> &Self {
        assert_eq!(self.len(), expected, "diagnostic report length mismatch",);

        self
    }

    fn assert_empty(&self) -> &Self {
        assert!(
            self.is_empty(),
            "expected an empty diagnostic report, got {} diagnostics",
            self.len(),
        );

        self
    }

    fn assert_not_empty(&self) -> &Self {
        assert!(!self.is_empty(), "expected a non-empty diagnostic report",);

        self
    }

    fn assert_status(&self, expected: ReportStatus) -> &Self {
        let actual = self.status();

        assert_eq!(actual, expected, "diagnostic report status mismatch");

        self
    }

    fn assert_counts(&self, expected: SeverityCounts) -> &Self {
        let actual = self.counts();

        assert_eq!(
            actual, expected,
            "diagnostic report severity counts mismatch",
        );

        self
    }

    fn assert_contains_severity(&self, severity: Severity) -> &Self {
        assert!(
            self.iter()
                .any(|diagnostic| diagnostic.severity == severity),
            "diagnostic report did not contain severity {severity}",
        );

        self
    }

    fn assert_contains_code(&self, expected: &str) -> &Self {
        assert!(
            self.iter()
                .any(|diagnostic| diagnostic.code.as_deref() == Some(expected)),
            "diagnostic report did not contain code {expected:?}",
        );

        self
    }

    fn assert_count_at_or_above(&self, minimum: Severity, expected: usize) -> &Self {
        let actual = self.count_at_or_above(minimum);

        assert_eq!(
            actual, expected,
            "unexpected number of diagnostics at or above {minimum}",
        );

        self
    }

    fn assert_none_at_or_above(&self, minimum: Severity) -> &Self {
        let count = self.count_at_or_above(minimum);

        assert_eq!(count, 0, "expected no diagnostics at or above {minimum}",);

        self
    }
}

/// Common imports for tests using `diagprint-test`.
pub mod prelude {
    pub use crate::{
        DiagnosticAssertions, DiagnosticReportAssertions, assert_diagnostic_snapshot,
        assert_report_snapshot,
    };
}
