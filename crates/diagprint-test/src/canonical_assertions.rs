use diagprint::{
    DeltaCounts, DeltaKind, Diagnostic, DiagnosticDelta, DiagnosticDigest, DiagnosticReport,
    ReportDigest, Severity,
};

/// Canonical identity assertions for one [`struct@Diagnostic`].
pub trait DiagnosticCanonicalAssertions {
    /// Asserts that two diagnostics represent the same logical diagnostic.
    fn assert_same_fingerprint_as(&self, other: &Diagnostic) -> &Self;

    /// Asserts that two diagnostics represent different logical diagnostics.
    fn assert_different_fingerprint_from(&self, other: &Diagnostic) -> &Self;

    /// Asserts that two diagnostics have identical canonical content.
    fn assert_same_digest_as(&self, other: &Diagnostic) -> &Self;

    /// Asserts that two diagnostics have different canonical content.
    fn assert_different_digest_from(&self, other: &Diagnostic) -> &Self;
}

impl DiagnosticCanonicalAssertions for Diagnostic {
    fn assert_same_fingerprint_as(&self, other: &Diagnostic) -> &Self {
        let left = self.fingerprint();
        let right = other.fingerprint();

        assert_eq!(
            left,
            right,
            "diagnostic fingerprint mismatch:\n  left:  {}\n  right: {}",
            left.qualified(),
            right.qualified(),
        );

        self
    }

    fn assert_different_fingerprint_from(&self, other: &Diagnostic) -> &Self {
        let left = self.fingerprint();
        let right = other.fingerprint();

        assert_ne!(
            left,
            right,
            "expected different diagnostic fingerprints, but both were {}",
            left.qualified(),
        );

        self
    }

    fn assert_same_digest_as(&self, other: &Diagnostic) -> &Self {
        let left = diagnostic_digest(self);
        let right = diagnostic_digest(other);

        assert_eq!(
            left,
            right,
            "diagnostic digest mismatch:\n  left:  {}\n  right: {}",
            left.qualified(),
            right.qualified(),
        );

        self
    }

    fn assert_different_digest_from(&self, other: &Diagnostic) -> &Self {
        let left = diagnostic_digest(self);
        let right = diagnostic_digest(other);

        assert_ne!(
            left,
            right,
            "expected different diagnostic digests, but both were {}",
            left.qualified(),
        );

        self
    }
}

/// Canonical digest assertions for [`DiagnosticReport`].
pub trait DiagnosticReportCanonicalAssertions {
    /// Asserts that two reports contain identical canonical diagnostic content.
    ///
    /// Report insertion order does not affect the digest.
    fn assert_same_digest_as(&self, other: &DiagnosticReport) -> &Self;

    /// Asserts that two reports have different canonical diagnostic content.
    fn assert_different_digest_from(&self, other: &DiagnosticReport) -> &Self;
}

impl DiagnosticReportCanonicalAssertions for DiagnosticReport {
    fn assert_same_digest_as(&self, other: &DiagnosticReport) -> &Self {
        let left = report_digest(self);
        let right = report_digest(other);

        assert_eq!(
            left,
            right,
            "diagnostic report digest mismatch:\n  left:  {}\n  right: {}",
            left.qualified(),
            right.qualified(),
        );

        self
    }

    fn assert_different_digest_from(&self, other: &DiagnosticReport) -> &Self {
        let left = report_digest(self);
        let right = report_digest(other);

        assert_ne!(
            left,
            right,
            "expected different diagnostic report digests, but both were {}",
            left.qualified(),
        );

        self
    }
}

/// Assertions over a semantic [`DiagnosticDelta`].
pub trait DiagnosticDeltaAssertions {
    /// Asserts exact delta classification counts.
    fn assert_delta_counts(&self, expected: DeltaCounts) -> &Self;

    /// Asserts that the baseline and candidate have no semantic differences.
    fn assert_unchanged(&self) -> &Self;

    /// Asserts that the delta contains at least one semantic difference.
    fn assert_has_differences(&self) -> &Self;

    /// Asserts the exact number of entries with `kind`.
    fn assert_kind_count(&self, kind: DeltaKind, expected: usize) -> &Self;

    /// Asserts how many candidate diagnostics newly cross `minimum`.
    fn assert_introduced_at_or_above(&self, minimum: Severity, expected: usize) -> &Self;

    /// Asserts the number of changed diagnostics whose severity increased.
    fn assert_severity_increases(&self, expected: usize) -> &Self;

    /// Asserts the number of severity increases reaching at least `minimum`.
    fn assert_severity_increases_at_or_above(&self, minimum: Severity, expected: usize) -> &Self;
}

impl DiagnosticDeltaAssertions for DiagnosticDelta {
    fn assert_delta_counts(&self, expected: DeltaCounts) -> &Self {
        let actual = self.counts();

        assert_eq!(actual, expected, "diagnostic delta counts mismatch",);

        self
    }

    fn assert_unchanged(&self) -> &Self {
        assert!(
            self.is_unchanged(),
            "expected an unchanged diagnostic delta, got {:?}",
            self.counts(),
        );

        self
    }

    fn assert_has_differences(&self) -> &Self {
        assert!(
            self.has_differences(),
            "expected diagnostic delta differences, but the delta was unchanged",
        );

        self
    }

    fn assert_kind_count(&self, kind: DeltaKind, expected: usize) -> &Self {
        let actual = self.iter_kind(kind).count();

        assert_eq!(
            actual, expected,
            "unexpected number of {kind:?} diagnostic delta entries",
        );

        self
    }

    fn assert_introduced_at_or_above(&self, minimum: Severity, expected: usize) -> &Self {
        let actual = self.introduced_at_or_above(minimum);

        assert_eq!(
            actual, expected,
            "unexpected number of diagnostics introduced at or above {minimum}",
        );

        self
    }

    fn assert_severity_increases(&self, expected: usize) -> &Self {
        let actual = self.severity_increases();

        assert_eq!(
            actual, expected,
            "unexpected number of diagnostic severity increases",
        );

        self
    }

    fn assert_severity_increases_at_or_above(&self, minimum: Severity, expected: usize) -> &Self {
        let actual = self.severity_increases_at_or_above(minimum);

        assert_eq!(
            actual, expected,
            "unexpected number of severity increases at or above {minimum}",
        );

        self
    }
}

fn diagnostic_digest(diagnostic: &Diagnostic) -> DiagnosticDigest {
    diagnostic
        .digest()
        .unwrap_or_else(|error| panic!("failed to compute canonical diagnostic digest: {error}"))
}

fn report_digest(report: &DiagnosticReport) -> ReportDigest {
    report.digest().unwrap_or_else(|error| {
        panic!("failed to compute canonical diagnostic report digest: {error}")
    })
}
