use diagprint::{
    DiagnosticHistory, DiagnosticRelationship, DiagnosticRelationshipEvidence,
    DiagnosticRelationshipGraph, DiagnosticRelationshipKind, DiagnosticRelationshipSnapshot,
    DiagnosticReport, Reporter,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-relationship-cli-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diagprint"))
        .args(args)
        .output()
        .expect("diagprint should execute")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn build_fixture(root: &Path) -> (String, String, String) {
    let reporter = Reporter::builder()
        .application("relationship-cli-test")
        .build()
        .unwrap();

    let first = reporter.error("first diagnostic");
    let second = reporter.error("second diagnostic");
    let third = reporter.warning("third diagnostic");

    let first_fp = first.fingerprint().qualified();
    let second_fp = second.fingerprint().qualified();
    let third_fp = third.fingerprint().qualified();

    let mut report = DiagnosticReport::new();
    report.push(first).push(second).push(third);

    let graph = DiagnosticRelationshipGraph::from_report(
        &report,
        [
            DiagnosticRelationship::new(
                first_fp.clone(),
                second_fp.clone(),
                DiagnosticRelationshipKind::Causes,
                DiagnosticRelationshipEvidence::ProducerDeclared,
                "diagprint.native",
            )
            .unwrap(),
            DiagnosticRelationship::new(
                second_fp.clone(),
                third_fp.clone(),
                DiagnosticRelationshipKind::DependsOn,
                DiagnosticRelationshipEvidence::Structural,
                "diagprint.native",
            )
            .unwrap(),
            DiagnosticRelationship::new(
                first_fp.clone(),
                third_fp.clone(),
                DiagnosticRelationshipKind::CoOccursWith,
                DiagnosticRelationshipEvidence::InferredCorrelation,
                "diagprint.native",
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let mut history = DiagnosticHistory::open(root).unwrap();
    let run = history.append_report("graph-run", &report).unwrap().clone();

    DiagnosticRelationshipSnapshot::new(&run, graph)
        .unwrap()
        .persist(&history)
        .unwrap();

    (first_fp, second_fp, third_fp)
}

fn short(fingerprint: &str) -> &str {
    let hex = fingerprint.rsplit(':').next().unwrap();
    &hex[..12]
}

#[test]
fn graph_cli_supports_text_json_dot_and_unique_prefixes() {
    let root = temporary_directory("formats");
    fs::create_dir_all(&root).unwrap();

    let (first, second, third) = build_fixture(&root);

    let root_text = root.to_string_lossy().into_owned();

    let output = run(&["graph", &root_text, short(&second), "--depth", "3"]);

    assert!(
        output.status.success(),
        "graph text failed:\n{}",
        text(&output)
    );

    let rendered = text(&output);

    assert!(rendered.contains("DIAGNOSTIC RELATIONSHIP GRAPH"));
    assert!(rendered.contains("history-binding-verified: true"));
    assert!(rendered.contains("EXPLICIT RELATIONSHIPS"));
    assert!(rendered.contains("STRUCTURAL / TRACE RELATIONSHIPS"));
    assert!(rendered.contains("INFERRED CORRELATIONS\n  none"));
    assert!(rendered.contains("independent-root-cause: NOT ESTABLISHED"));
    assert!(rendered.contains(short(&first)));
    assert!(rendered.contains(short(&third)));

    let json_output = run(&[
        "history",
        "graph",
        &root_text,
        short(&second),
        "--evidence",
        "all",
        "--format",
        "json",
    ]);

    assert!(
        json_output.status.success(),
        "graph JSON failed:\n{}",
        text(&json_output)
    );

    let json: Value = serde_json::from_slice(&json_output.stdout).unwrap();

    assert_eq!(json["root"].as_str(), Some(second.as_str()));
    assert_eq!(json["evidence"].as_str(), Some("all"));
    assert_eq!(json["graph"]["edges"].as_array().unwrap().len(), 3);

    let dot_output = run(&[
        "graph",
        &root_text,
        short(&second),
        "--evidence",
        "all",
        "--format",
        "dot",
    ]);

    assert!(
        dot_output.status.success(),
        "graph DOT failed:\n{}",
        text(&dot_output)
    );

    let dot = String::from_utf8(dot_output.stdout).unwrap();

    assert!(dot.starts_with("digraph diagprint {\n"));
    assert!(dot.contains("causes / producer_declared / diagprint.native"));
    assert!(dot.contains("co_occurs_with / inferred_correlation / diagprint.native"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn graph_cli_rejects_missing_snapshot_and_out_of_range_run() {
    let root = temporary_directory("missing");
    fs::create_dir_all(&root).unwrap();

    let reporter = Reporter::builder()
        .application("relationship-cli-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("no graph snapshot");
    let fingerprint = diagnostic.fingerprint().qualified();

    let mut report = DiagnosticReport::new();
    report.push(diagnostic);

    let mut history = DiagnosticHistory::open(&root).unwrap();
    history.append_report("run", &report).unwrap();

    let root_text = root.to_string_lossy().into_owned();

    let missing = run(&["graph", &root_text, short(&fingerprint)]);

    assert!(!missing.status.success());
    assert!(text(&missing).contains("no retained relationship snapshot"));

    let bad_run = run(&["graph", &root_text, short(&fingerprint), "--run", "99"]);

    assert!(!bad_run.status.success());
    assert!(text(&bad_run).contains("out of range"));

    fs::remove_dir_all(root).unwrap();
}
