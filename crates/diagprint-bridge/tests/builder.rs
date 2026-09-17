use diagprint::{
    Cause, DiagnosticRelationshipEvidence, DiagnosticRelationshipKind, DocumentationLink,
    IDENTITY_ATTRIBUTE, InteropLabel, Reporter, Severity,
};
use diagprint_bridge::{BridgeDiagnosticMetadata, BridgeError, BridgeOutputBuilder};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-bridge-test")
        .color(false)
        .build()
        .unwrap()
}

#[test]
fn metadata_reuses_interop_payload_and_identity() {
    let reporter = reporter();

    let metadata = BridgeDiagnosticMetadata::new("parser failed")
        .severity(Severity::Warning)
        .code("P100")
        .help("check the input")
        .note("adapter note")
        .label(
            InteropLabel::primary("virtual.rs", 7)
                .column(4)
                .length(3)
                .message("bad token"),
        )
        .cause(Cause::new("upstream parse source"))
        .documentation(DocumentationLink::new(
            "Parser docs",
            "https://example.invalid/parser",
        ))
        .identity("demo.parser")
        .unwrap();

    let mut builder = BridgeOutputBuilder::new(&reporter, "demo-adapter").unwrap();
    builder.push(metadata).unwrap();

    let output = builder.finish().unwrap();
    let diagnostic = output.report().iter().next().unwrap();

    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.code.as_deref(), Some("P100"));
    assert_eq!(diagnostic.help.as_deref(), Some("check the input"));
    assert!(diagnostic.notes.iter().any(|note| note == "adapter note"));
    assert_eq!(diagnostic.labels.len(), 1);
    assert!(diagnostic.cause.is_some());

    assert!(
        diagnostic
            .attributes
            .iter()
            .any(|attribute| { attribute.name == IDENTITY_ATTRIBUTE })
    );

    assert_eq!(output.stats().diagnostic_instances, 1);
    assert_eq!(output.stats().logical_nodes, 1);
}

#[test]
fn invalid_identity_and_producer_fail_closed() {
    assert!(matches!(
        BridgeDiagnosticMetadata::new("x").identity(""),
        Err(BridgeError::InvalidIdentity { .. })
    ));

    assert!(matches!(
        BridgeDiagnosticMetadata::new("x").identity("bad\nidentity"),
        Err(BridgeError::InvalidIdentity { .. })
    ));

    assert!(matches!(
        BridgeOutputBuilder::new(&reporter(), "producer with spaces"),
        Err(BridgeError::InvalidProducer { .. })
    ));
}

#[test]
fn node_handles_are_scoped_to_their_builder() {
    let reporter = reporter();

    let mut first = BridgeOutputBuilder::new(&reporter, "adapter-one").unwrap();
    let foreign = first
        .push(BridgeDiagnosticMetadata::new("foreign"))
        .unwrap();

    let mut second = BridgeOutputBuilder::new(&reporter, "adapter-two").unwrap();
    let local = second.push(BridgeDiagnosticMetadata::new("local")).unwrap();

    let error = second
        .relate(
            foreign,
            local,
            DiagnosticRelationshipKind::RelatedTo,
            DiagnosticRelationshipEvidence::ProducerDeclared,
        )
        .err()
        .expect("foreign bridge node should fail");

    assert!(matches!(error, BridgeError::ForeignNode { .. }));
}

#[test]
fn m4_relation_validation_is_reused_by_every_adapter() {
    let reporter = reporter();
    let mut builder = BridgeOutputBuilder::new(&reporter, "demo-adapter").unwrap();

    let left = builder.push(BridgeDiagnosticMetadata::new("left")).unwrap();
    let right = builder
        .push(BridgeDiagnosticMetadata::new("right"))
        .unwrap();

    let error = builder
        .relate(
            left,
            right,
            DiagnosticRelationshipKind::Causes,
            DiagnosticRelationshipEvidence::Structural,
        )
        .err()
        .expect("invalid causal evidence should fail");

    assert!(matches!(error, BridgeError::Relationship(_)));
}

#[test]
fn duplicate_logical_instances_collapse_self_relation_centrally() {
    let reporter = reporter();
    let mut builder = BridgeOutputBuilder::new(&reporter, "demo-adapter").unwrap();

    let inner = builder
        .push(
            BridgeDiagnosticMetadata::new("inner wording")
                .identity("demo.same")
                .unwrap(),
        )
        .unwrap();

    let outer = builder
        .push(
            BridgeDiagnosticMetadata::new("outer wording")
                .identity("demo.same")
                .unwrap(),
        )
        .unwrap();

    builder
        .relate(
            inner,
            outer,
            DiagnosticRelationshipKind::ContributesTo,
            DiagnosticRelationshipEvidence::SourceChain,
        )
        .unwrap();

    let output = builder.finish().unwrap();

    assert_eq!(output.report().len(), 2);
    assert_eq!(output.graph().node_count(), 1);
    assert_eq!(output.graph().edge_count(), 0);
    assert_eq!(output.stats().diagnostic_instances, 2);
    assert_eq!(output.stats().logical_nodes, 1);
    assert_eq!(output.stats().relationships, 0);
    assert_eq!(output.stats().collapsed_self_relationships, 1);
}

fn build_deterministic_graph(reverse_push_order: bool) -> String {
    let reporter = reporter();
    let mut builder = BridgeOutputBuilder::new(&reporter, "demo-adapter").unwrap();

    let (alpha, beta) = if reverse_push_order {
        let beta = builder
            .push(
                BridgeDiagnosticMetadata::new("beta")
                    .identity("demo.beta")
                    .unwrap(),
            )
            .unwrap();

        let alpha = builder
            .push(
                BridgeDiagnosticMetadata::new("alpha")
                    .identity("demo.alpha")
                    .unwrap(),
            )
            .unwrap();

        (alpha, beta)
    } else {
        let alpha = builder
            .push(
                BridgeDiagnosticMetadata::new("alpha")
                    .identity("demo.alpha")
                    .unwrap(),
            )
            .unwrap();

        let beta = builder
            .push(
                BridgeDiagnosticMetadata::new("beta")
                    .identity("demo.beta")
                    .unwrap(),
            )
            .unwrap();

        (alpha, beta)
    };

    builder
        .relate(
            alpha,
            beta,
            DiagnosticRelationshipKind::DependsOn,
            DiagnosticRelationshipEvidence::Structural,
        )
        .unwrap();

    builder.finish().unwrap().graph().graph_digest.to_string()
}

#[test]
fn graph_identity_does_not_depend_on_push_order() {
    assert_eq!(
        build_deterministic_graph(false),
        build_deterministic_graph(true)
    );
}
