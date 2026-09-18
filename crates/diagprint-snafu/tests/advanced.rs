use diagprint::{DiagnosticRelationshipEvidence, DiagnosticRelationshipKind, Reporter};
use diagprint_snafu::{
    SnafuBacktracePolicy, SnafuBridge, SnafuBridgeError, SnafuCaptureProfile, SnafuCode,
    SnafuDiagnostic, SnafuDiagnosticMetadata, SnafuErrorView, SnafuIdentity, SnafuTextPolicy,
    snafu_mapper,
};
use snafu::{Backtrace, ErrorCompat, GenerateImplicitData, Snafu};
use std::{error::Error as StdError, fmt};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-snafu-advanced-test")
        .color(false)
        .build()
        .unwrap()
}

#[derive(Debug, Snafu)]
#[snafu(display("Foreign value {value}"))]
struct ForeignError {
    value: u8,
}

#[test]
fn closure_mapper_and_profile_support_foreign_root() {
    let mapper = snafu_mapper(|view: SnafuErrorView<'_>| {
        if let Some(error) = view.downcast_ref::<ForeignError>() {
            Ok(Some(
                SnafuDiagnosticMetadata::new(
                    SnafuIdentity::new("foreign.value")?,
                    format!("Foreign value {}", error.value),
                )
                .code(SnafuCode::new("FOREIGN_VALUE")?),
            ))
        } else {
            Ok(None)
        }
    });

    let profile = SnafuCaptureProfile::new(mapper);
    let output = SnafuBridge::new()
        .convert_mapped_with_profile(&ForeignError { value: 7 }, &reporter(), &profile)
        .unwrap();

    assert_eq!(output.mapped_nodes(), 1);
    assert_eq!(
        output.report().iter().next().unwrap().code.as_deref(),
        Some("FOREIGN_VALUE")
    );
}

#[derive(Debug)]
struct SecretSource(&'static str);

impl fmt::Display for SecretSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl StdError for SecretSource {}

#[derive(Debug, Snafu)]
#[snafu(display("private root"))]
struct PrivateRoot {
    source: SecretSource,
}

impl SnafuDiagnostic for PrivateRoot {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Ok(
            SnafuDiagnosticMetadata::new(SnafuIdentity::new("privacy.root")?, "private root")
                .code(SnafuCode::new("PRIVACY_ROOT")?),
        )
    }
}

fn root_fingerprint(output: &diagprint_snafu::SnafuBridgeOutput) -> String {
    output
        .report()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("PRIVACY_ROOT"))
        .unwrap()
        .fingerprint()
        .qualified()
}

#[test]
fn redact_unmapped_removes_secret_without_changing_root_identity_or_topology() {
    const SECRET: &str = "sentinel-secret-do-not-export";

    let error = PrivateRoot {
        source: SecretSource(SECRET),
    };

    let display = SnafuBridge::new().convert(&error, &reporter()).unwrap();

    let redacted_profile =
        SnafuCaptureProfile::default().text_policy(SnafuTextPolicy::RedactUnmapped);

    let redacted = SnafuBridge::new()
        .convert_with_profile(&error, &reporter(), &redacted_profile)
        .unwrap();

    assert!(
        display
            .report()
            .iter()
            .any(|diagnostic| diagnostic.message.contains(SECRET))
    );

    assert!(
        redacted
            .report()
            .iter()
            .all(|diagnostic| !diagnostic.message.contains(SECRET))
    );

    assert_eq!(redacted.redacted_nodes(), 1);
    assert_eq!(root_fingerprint(&display), root_fingerprint(&redacted));
    assert_eq!(display.graph().node_count(), redacted.graph().node_count());
    assert_eq!(display.graph().edge_count(), redacted.graph().edge_count());

    let edge = redacted.graph().edges.first().unwrap();
    assert_eq!(edge.kind, DiagnosticRelationshipKind::ContributesTo);
    assert_eq!(edge.evidence, DiagnosticRelationshipEvidence::SourceChain);
}

struct BacktraceRoot {
    backtrace: Backtrace,
}

impl fmt::Debug for BacktraceRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BacktraceRoot").finish_non_exhaustive()
    }
}

impl fmt::Display for BacktraceRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("backtrace root")
    }
}

impl StdError for BacktraceRoot {}

impl ErrorCompat for BacktraceRoot {
    fn backtrace(&self) -> Option<&Backtrace> {
        Some(&self.backtrace)
    }
}

impl SnafuDiagnostic for BacktraceRoot {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError> {
        Ok(
            SnafuDiagnosticMetadata::new(SnafuIdentity::new("backtrace.root")?, "backtrace root")
                .code(SnafuCode::new("BACKTRACE_ROOT")?),
        )
    }
}

fn backtrace_root_fingerprint(output: &diagprint_snafu::SnafuBridgeOutput) -> String {
    output
        .report()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("BACKTRACE_ROOT"))
        .unwrap()
        .fingerprint()
        .qualified()
}

#[test]
fn backtrace_is_omitted_by_default_and_opt_in_is_presentation_only() {
    let error = BacktraceRoot {
        backtrace: <Backtrace as GenerateImplicitData>::generate(),
    };

    let default_output = SnafuBridge::new().convert(&error, &reporter()).unwrap();

    let profile =
        SnafuCaptureProfile::default().backtrace_policy(SnafuBacktracePolicy::DisplayText);

    let with_backtrace = SnafuBridge::new()
        .convert_with_profile(&error, &reporter(), &profile)
        .unwrap();

    let default_root = default_output.report().iter().next().unwrap();
    let opt_in_root = with_backtrace.report().iter().next().unwrap();

    assert!(
        default_root
            .notes
            .iter()
            .all(|note| !note.starts_with("SNAFU backtrace:\n"))
    );

    assert!(
        opt_in_root
            .notes
            .iter()
            .any(|note| note.starts_with("SNAFU backtrace:\n"))
    );

    assert_eq!(default_output.backtraces_included(), 0);
    assert_eq!(with_backtrace.backtraces_included(), 1);
    assert_eq!(
        backtrace_root_fingerprint(&default_output),
        backtrace_root_fingerprint(&with_backtrace)
    );
}
