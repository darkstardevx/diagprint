use diagprint::{
    DiagnosticHistory, DiagnosticRelationship, DiagnosticRelationshipError,
    DiagnosticRelationshipEvidence, DiagnosticRelationshipGraph,
    DiagnosticRelationshipGraphBuilder, DiagnosticRelationshipKind, DiagnosticRelationshipSnapshot,
    DiagnosticReport, Reporter,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn fingerprint(hex: char) -> String {
    format!(
        "diagprint.canonical/v1:sha256:{}",
        std::iter::repeat_n(hex, 64).collect::<String>()
    )
}

fn temp_directory(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();

    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!(
        "diagprint-m4-{label}-{}-{nanos}-{counter}",
        std::process::id()
    ))
}

#[test]
fn graph_identity_is_deterministic_and_duplicate_safe() {
    let a = fingerprint('a');
    let b = fingerprint('b');
    let c = fingerprint('c');

    let first = DiagnosticRelationship::new(
        a.clone(),
        b.clone(),
        DiagnosticRelationshipKind::Causes,
        DiagnosticRelationshipEvidence::ProducerDeclared,
        "diagprint.native",
    )
    .expect("explicit causal relationship should be valid");

    let second = DiagnosticRelationship::new(
        b.clone(),
        c.clone(),
        DiagnosticRelationshipKind::DependsOn,
        DiagnosticRelationshipEvidence::Structural,
        "diagprint.native",
    )
    .expect("structural dependency should be valid");

    let mut left = DiagnosticRelationshipGraphBuilder::new();
    left.add_fingerprint(c.clone()).unwrap();
    left.add_relationship(second.clone()).unwrap();
    left.add_relationship(first.clone()).unwrap();
    left.add_relationship(first.clone()).unwrap();

    let mut right = DiagnosticRelationshipGraphBuilder::new();
    right.add_relationship(first).unwrap();
    right.add_fingerprint(c).unwrap();
    right.add_relationship(second).unwrap();

    let left = left.build().unwrap();
    let right = right.build().unwrap();

    assert_eq!(left, right);
    assert_eq!(left.node_count(), 3);
    assert_eq!(left.edge_count(), 2);
    assert_eq!(left.graph_digest, right.graph_digest);
    left.verify().unwrap();
}

#[test]
fn inferred_and_temporal_evidence_cannot_assert_causation() {
    let a = fingerprint('a');
    let b = fingerprint('b');

    for evidence in [
        DiagnosticRelationshipEvidence::InferredCorrelation,
        DiagnosticRelationshipEvidence::TemporalAssociation,
        DiagnosticRelationshipEvidence::TraceContext,
        DiagnosticRelationshipEvidence::Structural,
    ] {
        let error = DiagnosticRelationship::new(
            a.clone(),
            b.clone(),
            DiagnosticRelationshipKind::Causes,
            evidence,
            "diagprint.native",
        )
        .expect_err("noncausal evidence must not assert causes");

        assert!(matches!(
            error,
            DiagnosticRelationshipError::CausalEvidenceMismatch { .. }
        ));
    }

    DiagnosticRelationship::new(
        a,
        b,
        DiagnosticRelationshipKind::CoOccursWith,
        DiagnosticRelationshipEvidence::InferredCorrelation,
        "diagprint.native",
    )
    .expect("inference may state correlation");
}

#[test]
fn source_chain_can_assert_explicit_causal_structure() {
    DiagnosticRelationship::new(
        fingerprint('a'),
        fingerprint('b'),
        DiagnosticRelationshipKind::ContributesTo,
        DiagnosticRelationshipEvidence::SourceChain,
        "error-stack",
    )
    .expect("source-chain evidence may explicitly state causal structure");
}

#[test]
fn symmetric_relationships_have_one_canonical_orientation() {
    let a = fingerprint('a');
    let b = fingerprint('b');

    let forward = DiagnosticRelationship::new(
        a.clone(),
        b.clone(),
        DiagnosticRelationshipKind::RelatedTo,
        DiagnosticRelationshipEvidence::ProducerDeclared,
        "diagprint.native",
    )
    .unwrap();

    let reverse = DiagnosticRelationship::new(
        b,
        a,
        DiagnosticRelationshipKind::RelatedTo,
        DiagnosticRelationshipEvidence::ProducerDeclared,
        "diagprint.native",
    )
    .unwrap();

    assert_eq!(forward, reverse);
}

#[test]
fn invalid_producers_and_self_edges_fail_closed() {
    let a = fingerprint('a');
    let b = fingerprint('b');

    assert!(matches!(
        DiagnosticRelationship::new(
            a.clone(),
            b,
            DiagnosticRelationshipKind::RelatedTo,
            DiagnosticRelationshipEvidence::ProducerDeclared,
            "producer with spaces",
        ),
        Err(DiagnosticRelationshipError::InvalidProducer { .. })
    ));

    assert!(matches!(
        DiagnosticRelationship::new(
            a.clone(),
            a,
            DiagnosticRelationshipKind::RelatedTo,
            DiagnosticRelationshipEvidence::ProducerDeclared,
            "diagprint.native",
        ),
        Err(DiagnosticRelationshipError::SelfRelationship { .. })
    ));
}

#[test]
fn report_builder_keeps_isolated_diagnostics_as_graph_nodes() {
    let reporter = Reporter::builder()
        .application("relationship-test")
        .build()
        .unwrap();

    let first = reporter.error("alpha");
    let second = reporter.warning("beta");

    let first_fingerprint = first.fingerprint().qualified();
    let second_fingerprint = second.fingerprint().qualified();

    let mut report = DiagnosticReport::new();
    report.push(first).push(second);

    let graph = DiagnosticRelationshipGraph::from_report(&report, []).unwrap();

    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edge_count(), 0);
    assert!(graph.contains(&first_fingerprint));
    assert!(graph.contains(&second_fingerprint));
}

#[test]
fn snapshot_round_trip_is_exact_idempotent_and_privacy_light() {
    let directory = temp_directory("round-trip");

    let reporter = Reporter::builder()
        .application("private-application")
        .build()
        .unwrap();

    let first = reporter.error("PRIVATE MESSAGE ALPHA");
    let second = reporter.warning("PRIVATE MESSAGE BETA");

    let first_fingerprint = first.fingerprint().qualified();
    let second_fingerprint = second.fingerprint().qualified();

    let relationship = DiagnosticRelationship::new(
        first_fingerprint,
        second_fingerprint,
        DiagnosticRelationshipKind::DependsOn,
        DiagnosticRelationshipEvidence::Structural,
        "diagprint.native",
    )
    .unwrap();

    let mut report = DiagnosticReport::new();
    report.push(first).push(second);

    let graph = DiagnosticRelationshipGraph::from_report(&report, [relationship]).unwrap();

    let mut history = DiagnosticHistory::open(&directory).unwrap();
    let run = history.append_report("run-0", &report).unwrap().clone();

    let snapshot = DiagnosticRelationshipSnapshot::new(&run, graph).unwrap();

    let first_path = snapshot.persist(&history).unwrap();
    let second_path = snapshot.persist(&history).unwrap();

    assert_eq!(first_path, second_path);

    let loaded = DiagnosticRelationshipSnapshot::load(&history, 0)
        .unwrap()
        .expect("snapshot should exist");

    assert_eq!(snapshot, loaded);

    let serialized = fs::read_to_string(&first_path).unwrap();

    assert!(!serialized.contains("PRIVATE MESSAGE ALPHA"));
    assert!(!serialized.contains("PRIVATE MESSAGE BETA"));
    assert!(!serialized.contains("private-application"));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn snapshot_requires_exact_history_fingerprint_set() {
    let directory = temp_directory("binding");

    let reporter = Reporter::builder()
        .application("relationship-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("one");
    let mut report = DiagnosticReport::new();
    report.push(diagnostic);

    let mut history = DiagnosticHistory::open(&directory).unwrap();
    let run = history.append_report("run-0", &report).unwrap().clone();

    let mut builder = DiagnosticRelationshipGraphBuilder::new();
    builder.add_fingerprint(fingerprint('f')).unwrap();
    let graph = builder.build().unwrap();

    assert!(matches!(
        DiagnosticRelationshipSnapshot::new(&run, graph),
        Err(DiagnosticRelationshipError::MissingObservedFingerprint { .. })
            | Err(DiagnosticRelationshipError::UnexpectedFingerprint { .. })
    ));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn tampered_snapshot_is_rejected() {
    let directory = temp_directory("tamper");

    let reporter = Reporter::builder()
        .application("relationship-test")
        .build()
        .unwrap();

    let first = reporter.error("alpha");
    let second = reporter.warning("beta");

    let relationship = DiagnosticRelationship::new(
        first.fingerprint().qualified(),
        second.fingerprint().qualified(),
        DiagnosticRelationshipKind::RelatedTo,
        DiagnosticRelationshipEvidence::ProducerDeclared,
        "diagprint.native",
    )
    .unwrap();

    let mut report = DiagnosticReport::new();
    report.push(first).push(second);

    let graph = DiagnosticRelationshipGraph::from_report(&report, [relationship]).unwrap();

    let mut history = DiagnosticHistory::open(&directory).unwrap();
    let run = history.append_report("run-0", &report).unwrap().clone();

    let snapshot = DiagnosticRelationshipSnapshot::new(&run, graph).unwrap();
    let path = snapshot.persist(&history).unwrap();

    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();

    json["graph"]["edges"][0]["producer"] = serde_json::Value::String("tampered".to_owned());

    fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

    assert!(DiagnosticRelationshipSnapshot::load(&history, 0).is_err());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn conflicting_snapshot_replacement_is_rejected() {
    let directory = temp_directory("conflict");

    let reporter = Reporter::builder()
        .application("relationship-test")
        .build()
        .unwrap();

    let first = reporter.error("alpha");
    let second = reporter.warning("beta");

    let first_fingerprint = first.fingerprint().qualified();
    let second_fingerprint = second.fingerprint().qualified();

    let mut report = DiagnosticReport::new();
    report.push(first).push(second);

    let mut history = DiagnosticHistory::open(&directory).unwrap();
    let run = history.append_report("run-0", &report).unwrap().clone();

    let graph_one = DiagnosticRelationshipGraph::from_report(
        &report,
        [DiagnosticRelationship::new(
            first_fingerprint.clone(),
            second_fingerprint.clone(),
            DiagnosticRelationshipKind::DependsOn,
            DiagnosticRelationshipEvidence::Structural,
            "diagprint.native",
        )
        .unwrap()],
    )
    .unwrap();

    let graph_two = DiagnosticRelationshipGraph::from_report(
        &report,
        [DiagnosticRelationship::new(
            first_fingerprint,
            second_fingerprint,
            DiagnosticRelationshipKind::RelatedTo,
            DiagnosticRelationshipEvidence::ProducerDeclared,
            "diagprint.native",
        )
        .unwrap()],
    )
    .unwrap();

    let first_snapshot = DiagnosticRelationshipSnapshot::new(&run, graph_one).unwrap();
    first_snapshot.persist(&history).unwrap();

    let conflicting = DiagnosticRelationshipSnapshot::new(&run, graph_two).unwrap();

    assert!(matches!(
        conflicting.persist(&history),
        Err(DiagnosticRelationshipError::AlreadyExists { .. })
    ));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn traversal_is_depth_bounded_cycle_safe_and_deterministic() {
    use diagprint::{DiagnosticRelationshipDirection, DiagnosticRelationshipEvidenceFilter};

    let a = fingerprint('a');
    let b = fingerprint('b');
    let c = fingerprint('c');
    let d = fingerprint('d');

    let mut builder = DiagnosticRelationshipGraphBuilder::new();

    for value in [&a, &b, &c, &d] {
        builder.add_fingerprint(value.clone()).unwrap();
    }

    for (from, to) in [(&a, &b), (&b, &c), (&c, &a)] {
        builder
            .add_relationship(
                DiagnosticRelationship::new(
                    from.clone(),
                    to.clone(),
                    DiagnosticRelationshipKind::Causes,
                    DiagnosticRelationshipEvidence::ProducerDeclared,
                    "diagprint.native",
                )
                .unwrap(),
            )
            .unwrap();
    }

    builder
        .add_relationship(
            DiagnosticRelationship::new(
                c.clone(),
                d.clone(),
                DiagnosticRelationshipKind::CoOccursWith,
                DiagnosticRelationshipEvidence::InferredCorrelation,
                "diagprint.native",
            )
            .unwrap(),
        )
        .unwrap();

    let graph = builder.build().unwrap();

    let explicit = graph
        .subgraph(
            &a,
            DiagnosticRelationshipDirection::Both,
            10,
            DiagnosticRelationshipEvidenceFilter::Explicit,
        )
        .unwrap()
        .unwrap();

    assert_eq!(explicit.node_count(), 3);
    assert_eq!(explicit.edge_count(), 3);
    assert!(!explicit.contains(&d));

    let all = graph
        .subgraph(
            &a,
            DiagnosticRelationshipDirection::Both,
            10,
            DiagnosticRelationshipEvidenceFilter::All,
        )
        .unwrap()
        .unwrap();

    assert_eq!(all.node_count(), 4);
    assert_eq!(all.edge_count(), 4);

    let shallow = graph
        .subgraph(
            &a,
            DiagnosticRelationshipDirection::Downstream,
            1,
            DiagnosticRelationshipEvidenceFilter::Explicit,
        )
        .unwrap()
        .unwrap();

    assert_eq!(shallow.node_count(), 2);
    assert_eq!(shallow.edge_count(), 1);
}

#[test]
fn explicit_causal_cascade_analysis_ignores_noncausal_edges() {
    let a = fingerprint('a');
    let b = fingerprint('b');
    let c = fingerprint('c');
    let d = fingerprint('d');

    let mut builder = DiagnosticRelationshipGraphBuilder::new();

    for value in [&a, &b, &c, &d] {
        builder.add_fingerprint(value.clone()).unwrap();
    }

    builder
        .add_relationship(
            DiagnosticRelationship::new(
                a.clone(),
                b.clone(),
                DiagnosticRelationshipKind::Causes,
                DiagnosticRelationshipEvidence::ProducerDeclared,
                "diagprint.native",
            )
            .unwrap(),
        )
        .unwrap();

    builder
        .add_relationship(
            DiagnosticRelationship::new(
                b.clone(),
                c.clone(),
                DiagnosticRelationshipKind::ContributesTo,
                DiagnosticRelationshipEvidence::SourceChain,
                "error-stack",
            )
            .unwrap(),
        )
        .unwrap();

    builder
        .add_relationship(
            DiagnosticRelationship::new(
                c.clone(),
                d.clone(),
                DiagnosticRelationshipKind::DependsOn,
                DiagnosticRelationshipEvidence::Structural,
                "diagprint.native",
            )
            .unwrap(),
        )
        .unwrap();

    let graph = builder.build().unwrap();

    assert_eq!(
        graph.explicit_causal_upstream(&c, 8).unwrap().unwrap(),
        vec![a.clone(), b.clone()],
    );

    assert_eq!(
        graph.explicit_causal_downstream(&a, 8).unwrap().unwrap(),
        vec![b, c],
    );

    assert!(
        !graph
            .explicit_causal_downstream(&a, 8)
            .unwrap()
            .unwrap()
            .contains(&d)
    );
}

#[test]
fn dot_output_is_deterministic_and_preserves_edge_classification() {
    let a = fingerprint('a');
    let b = fingerprint('b');

    let mut builder = DiagnosticRelationshipGraphBuilder::new();

    builder
        .add_relationship(
            DiagnosticRelationship::new(
                a,
                b,
                DiagnosticRelationshipKind::Causes,
                DiagnosticRelationshipEvidence::ProducerDeclared,
                "diagprint.native",
            )
            .unwrap(),
        )
        .unwrap();

    let graph = builder.build().unwrap();

    let first = graph.to_dot().unwrap();
    let second = graph.to_dot().unwrap();

    assert_eq!(first, second);
    assert!(first.starts_with("digraph diagprint {\n"));
    assert!(first.contains("causes / producer_declared / diagprint.native"));
}
