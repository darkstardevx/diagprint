use diagprint::Reporter;
use diagprint_error_stack::ErrorStackReportExt;
use error_stack::Report;
use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Debug)]
struct RootError(&'static str);

impl fmt::Display for RootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "root {}", self.0)
    }
}

impl Error for RootError {}

#[derive(Debug)]
struct BranchError(&'static str);

impl fmt::Display for BranchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "branch {}", self.0)
    }
}

impl Error for BranchError {}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-error-stack-grouped-test")
        .color(false)
        .build()
        .unwrap()
}

fn grouped() -> Report<[BranchError]> {
    let mut report = Report::new(RootError("a"))
        .change_context(BranchError("a"))
        .expand();

    report.push(Report::new(RootError("b")).change_context(BranchError("b")));

    report
}

#[test]
fn grouped_report_preserves_each_branch_without_sibling_edges() {
    let output = grouped().to_diagprint(&reporter()).unwrap();

    assert_eq!(output.context_frames(), 4);
    assert_eq!(output.report().len(), 4);
    assert_eq!(output.graph().node_count(), 4);
    assert_eq!(output.graph().edge_count(), 2);

    let fingerprint_for = |message: &str| {
        output
            .report()
            .iter()
            .find(|diagnostic| diagnostic.message == message)
            .unwrap()
            .fingerprint()
            .qualified()
    };

    let root_a = fingerprint_for("root a");
    let branch_a = fingerprint_for("branch a");
    let root_b = fingerprint_for("root b");
    let branch_b = fingerprint_for("branch b");

    let edges = output
        .graph()
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect::<BTreeSet<_>>();

    assert!(edges.contains(&(root_a.as_str(), branch_a.as_str())));
    assert!(edges.contains(&(root_b.as_str(), branch_b.as_str())));

    assert!(!edges.contains(&(branch_a.as_str(), branch_b.as_str())));
    assert!(!edges.contains(&(branch_b.as_str(), branch_a.as_str())));
    assert!(!edges.contains(&(root_a.as_str(), branch_b.as_str())));
    assert!(!edges.contains(&(root_b.as_str(), branch_a.as_str())));
}

#[test]
fn grouped_report_uses_one_shared_sdk_output() {
    let output = grouped().to_diagprint(&reporter()).unwrap();

    assert_eq!(output.stats().diagnostic_instances, 4);
    assert_eq!(output.stats().logical_nodes, 4);
    assert_eq!(output.stats().relationships, 2);
    assert_eq!(output.stats().collapsed_self_relationships, 0);
}
