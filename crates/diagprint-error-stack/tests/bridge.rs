use diagprint::{
    DiagnosticRelationshipEvidence, DiagnosticRelationshipKind, IDENTITY_ATTRIBUTE, Reporter,
    Severity,
};
use diagprint_bridge::{BridgeDiagnosticMetadata, BridgeError};
use diagprint_error_stack::{
    ErrorStackBridge, ErrorStackContextMapper, ErrorStackContextView, ErrorStackReportExt,
};
use error_stack::Report;
use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Debug)]
struct RootError {
    value: &'static str,
}

impl fmt::Display for RootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "root failure: {}", self.value)
    }
}

impl Error for RootError {}

#[derive(Debug)]
struct MiddleError;

impl fmt::Display for MiddleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("middle failure")
    }
}

impl Error for MiddleError {}

#[derive(Debug)]
struct OuterError;

impl fmt::Display for OuterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("outer failure")
    }
}

impl Error for OuterError {}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-error-stack-test")
        .color(false)
        .build()
        .unwrap()
}

fn chain(value: &'static str) -> Report<OuterError> {
    Report::new(RootError { value })
        .change_context(MiddleError)
        .change_context(OuterError)
}

#[test]
fn stable_context_chain_uses_sdk_for_report_and_source_graph() {
    let output = chain("alpha").to_diagprint(&reporter()).unwrap();

    assert_eq!(output.context_frames(), 3);
    assert_eq!(output.report().len(), 3);
    assert_eq!(output.graph().node_count(), 3);
    assert_eq!(output.graph().edge_count(), 2);
    assert_eq!(output.stats().diagnostic_instances, 3);
    assert_eq!(output.stats().relationships, 2);

    output.graph().verify().unwrap();

    for edge in &output.graph().edges {
        assert_eq!(edge.kind, DiagnosticRelationshipKind::ContributesTo);
        assert_eq!(edge.evidence, DiagnosticRelationshipEvidence::SourceChain);
        assert_eq!(edge.producer, "error-stack");
    }

    assert!(
        output
            .graph()
            .edges
            .iter()
            .all(|edge| edge.kind != DiagnosticRelationshipKind::Causes)
    );

    let messages = output
        .report()
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<BTreeSet<_>>();

    assert!(messages.contains("root failure: alpha"));
    assert!(messages.contains("middle failure"));
    assert!(messages.contains("outer failure"));
}

#[test]
fn default_mapping_is_error_severity_without_external_identity() {
    let output = ErrorStackBridge::new()
        .convert(&chain("alpha"), &reporter())
        .unwrap();

    assert!(
        output
            .report()
            .iter()
            .all(|diagnostic| diagnostic.severity == Severity::Error)
    );

    assert!(output.report().iter().all(|diagnostic| {
        diagnostic
            .attributes
            .iter()
            .all(|attribute| attribute.name != IDENTITY_ATTRIBUTE)
    }));
}

struct TypedMapper;

impl ErrorStackContextMapper for TypedMapper {
    fn map(
        &self,
        view: ErrorStackContextView<'_>,
    ) -> Result<BridgeDiagnosticMetadata, BridgeError> {
        if let Some(root) = view.frame().downcast_ref::<RootError>() {
            return BridgeDiagnosticMetadata::new(root.to_string())
                .severity(Severity::Warning)
                .code("root-config")
                .help("repair the root configuration")
                .note("mapped through typed error-stack context")
                .identity("demo.root-config");
        }

        if view.frame().downcast_ref::<MiddleError>().is_some() {
            return BridgeDiagnosticMetadata::new(view.context().to_string())
                .code("middle-layer")
                .identity("demo.middle");
        }

        BridgeDiagnosticMetadata::new(view.context().to_string())
            .code("outer-layer")
            .identity("demo.outer")
    }
}

#[test]
fn typed_mapper_uses_shared_sdk_metadata_and_stable_identity() {
    let first = chain("volatile-a")
        .to_diagprint_with(&reporter(), &TypedMapper)
        .unwrap();

    let second = chain("volatile-b")
        .to_diagprint_with(&reporter(), &TypedMapper)
        .unwrap();

    let first_root = first
        .report()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("root-config"))
        .unwrap();

    let second_root = second
        .report()
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("root-config"))
        .unwrap();

    assert_eq!(first_root.severity, Severity::Warning);
    assert_eq!(
        first_root.help.as_deref(),
        Some("repair the root configuration")
    );
    assert!(
        first_root
            .notes
            .iter()
            .any(|note| note == "mapped through typed error-stack context")
    );

    assert_eq!(
        first_root.fingerprint().qualified(),
        second_root.fingerprint().qualified()
    );
}

struct IdentityMapper;

impl ErrorStackContextMapper for IdentityMapper {
    fn map(
        &self,
        view: ErrorStackContextView<'_>,
    ) -> Result<BridgeDiagnosticMetadata, BridgeError> {
        let identity = if view.frame().downcast_ref::<RootError>().is_some() {
            "demo.root"
        } else if view.frame().downcast_ref::<MiddleError>().is_some() {
            "demo.middle"
        } else {
            "demo.outer"
        };

        BridgeDiagnosticMetadata::new(view.context().to_string()).identity(identity)
    }
}

#[test]
fn source_edges_follow_root_to_middle_to_outer_direction() {
    let output = chain("alpha")
        .to_diagprint_with(&reporter(), &IdentityMapper)
        .unwrap();

    let fingerprint_for = |message: &str| {
        output
            .report()
            .iter()
            .find(|diagnostic| diagnostic.message == message)
            .unwrap()
            .fingerprint()
            .qualified()
    };

    let root = fingerprint_for("root failure: alpha");
    let middle = fingerprint_for("middle failure");
    let outer = fingerprint_for("outer failure");

    let edges = output
        .graph()
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect::<BTreeSet<_>>();

    assert!(edges.contains(&(root.as_str(), middle.as_str())));
    assert!(edges.contains(&(middle.as_str(), outer.as_str())));
    assert!(!edges.contains(&(outer.as_str(), middle.as_str())));
}

#[test]
fn attachment_frames_are_traversed_without_exporting_attachment_text() {
    const SECRET: &str = "TOP_SECRET_ERROR_STACK_ATTACHMENT";

    let report = Report::new(RootError { value: "attached" })
        .attach(SECRET.to_owned())
        .change_context(OuterError);

    let output = report.to_diagprint(&reporter()).unwrap();

    assert_eq!(output.context_frames(), 2);
    assert!(output.attachment_frames() >= 1);
    assert_eq!(output.graph().edge_count(), 1);

    for diagnostic in output.report() {
        assert!(!diagnostic.message.contains(SECRET));
        assert!(diagnostic.notes.iter().all(|note| !note.contains(SECRET)));
        assert!(
            diagnostic
                .help
                .as_deref()
                .is_none_or(|help| !help.contains(SECRET))
        );
    }
}
