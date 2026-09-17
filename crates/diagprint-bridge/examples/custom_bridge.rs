use diagprint::{DiagnosticRelationshipEvidence, DiagnosticRelationshipKind, Reporter, Severity};
use diagprint_bridge::{BridgeDiagnosticMetadata, BridgeOutputBuilder};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let reporter = Reporter::builder()
        .application("custom-bridge-example")
        .build()?;

    let mut bridge = BridgeOutputBuilder::new(&reporter, "custom-parser")?;

    let source = bridge.push(
        BridgeDiagnosticMetadata::new("invalid integer")
            .severity(Severity::Error)
            .code("PARSE001")
            .identity("custom-parser.invalid-integer")?,
    )?;

    let outer = bridge.push(
        BridgeDiagnosticMetadata::new("configuration could not be loaded")
            .code("CONFIG001")
            .identity("custom-parser.config-load")?,
    )?;

    bridge.relate(
        source,
        outer,
        DiagnosticRelationshipKind::ContributesTo,
        DiagnosticRelationshipEvidence::SourceChain,
    )?;

    let output = bridge.finish()?;

    reporter.emit_report(output.report())?;

    println!(
        "bridge graph: nodes={} edges={} collapsed-self={}",
        output.graph().node_count(),
        output.graph().edge_count(),
        output.stats().collapsed_self_relationships,
    );

    Ok(())
}
