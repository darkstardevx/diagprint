use diagprint::Reporter;
use diagprint_error_stack::{ErrorStackAttachmentPolicy, ErrorStackReportExt};
use error_stack::Report;
use std::{error::Error, fmt};

const PRINTABLE_SECRET: &str = "PRINTABLE_SECRET_9b73c46e";
const OPAQUE_SECRET: &str = "OPAQUE_SECRET_64d7a0f1";

#[derive(Debug)]
struct RootError;

impl fmt::Display for RootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("privacy root")
    }
}

impl Error for RootError {}

#[derive(Debug)]
struct OuterError;

impl fmt::Display for OuterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("privacy outer")
    }
}

impl Error for OuterError {}

fn reporter() -> Reporter {
    Reporter::builder()
        .application("diagprint-error-stack-privacy-test")
        .color(false)
        .build()
        .unwrap()
}

fn attached_report() -> Report<OuterError> {
    Report::new(RootError)
        .attach_opaque(OPAQUE_SECRET.to_owned())
        .attach(PRINTABLE_SECRET.to_owned())
        .change_context(OuterError)
}

fn serialized_diagnostics(output: &diagprint_error_stack::ErrorStackBridgeOutput) -> String {
    output
        .report()
        .iter()
        .map(|diagnostic| serde_json::to_string(diagnostic).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn default_policy_omits_printable_and_opaque_attachment_content() {
    let output = attached_report().to_diagprint(&reporter()).unwrap();
    let serialized = serialized_diagnostics(&output);

    assert!(output.attachment_frames() >= 2);
    assert!(output.printable_attachment_frames() >= 1);
    assert!(output.opaque_attachment_frames() >= 1);
    assert_eq!(output.included_printable_attachments(), 0);

    assert!(!serialized.contains(PRINTABLE_SECRET));
    assert!(!serialized.contains(OPAQUE_SECRET));
}

#[test]
fn printable_opt_in_includes_only_printable_attachment_text() {
    let output = attached_report()
        .to_diagprint_with_policy(&reporter(), ErrorStackAttachmentPolicy::PrintableText)
        .unwrap();

    let serialized = serialized_diagnostics(&output);

    assert!(serialized.contains(PRINTABLE_SECRET));
    assert!(!serialized.contains(OPAQUE_SECRET));
    assert_eq!(output.included_printable_attachments(), 1);
}

#[test]
fn printable_attachment_is_owned_by_nearest_context_below_it() {
    let output = attached_report()
        .to_diagprint_with_policy(&reporter(), ErrorStackAttachmentPolicy::PrintableText)
        .unwrap();

    let root = output
        .report()
        .iter()
        .find(|diagnostic| diagnostic.message == "privacy root")
        .unwrap();

    let outer = output
        .report()
        .iter()
        .find(|diagnostic| diagnostic.message == "privacy outer")
        .unwrap();

    assert!(
        root.notes
            .iter()
            .any(|note| note.contains(PRINTABLE_SECRET))
    );

    assert!(
        outer
            .notes
            .iter()
            .all(|note| !note.contains(PRINTABLE_SECRET))
    );
}

#[test]
fn attachment_policy_does_not_change_source_relationship_topology() {
    let omitted = attached_report().to_diagprint(&reporter()).unwrap();

    let included = attached_report()
        .to_diagprint_with_policy(&reporter(), ErrorStackAttachmentPolicy::PrintableText)
        .unwrap();

    assert_eq!(omitted.graph().node_count(), included.graph().node_count());
    assert_eq!(omitted.graph().edge_count(), included.graph().edge_count());
    assert_eq!(
        omitted.graph().graph_digest,
        included.graph().graph_digest,
        "notes are presentation enrichment and must not alter canonical graph identity"
    );
}
