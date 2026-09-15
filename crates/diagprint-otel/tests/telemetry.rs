use diagprint::{REDACTED, Reporter};
use diagprint_otel::{
    LocationExport, TelemetryAdapter, TelemetryEvent, TelemetryPolicy, TextExport,
};

fn string_attribute(event: &TelemetryEvent, key: &str) -> Option<String> {
    event
        .attributes()
        .iter()
        .find(|attribute| attribute.key.as_str() == key)
        .map(|attribute| attribute.value.as_str().into_owned())
}

fn has_attribute(event: &TelemetryEvent, key: &str) -> bool {
    event
        .attributes()
        .iter()
        .any(|attribute| attribute.key.as_str() == key)
}

#[test]
fn default_policy_does_not_export_sensitive_free_form_text() {
    let reporter = Reporter::builder()
        .application("telemetry-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("token=super-secret")
        .code("AUTH-001")
        .help("password=hunter2")
        .note("api-key=abcdef")
        .label(
            "/home/alice/private.rs",
            7,
            Some(3),
            Some(5),
            Some("private source message"),
        );

    let event = TelemetryAdapter::default().event(&diagnostic);

    assert_eq!(
        string_attribute(&event, "diagprint.message",),
        Some(REDACTED.to_owned())
    );

    assert_eq!(
        string_attribute(&event, "diagprint.code",),
        Some("AUTH-001".to_owned())
    );

    assert!(!has_attribute(&event, "diagprint.help",));

    assert!(!has_attribute(&event, "diagprint.note.0",));

    assert!(!has_attribute(&event, "diagprint.source.file",));

    assert!(!has_attribute(&event, "host.name",));

    assert!(!has_attribute(&event, "process.pid",));

    assert!(event.marks_span_error());
}

#[test]
fn plaintext_and_filename_export_require_explicit_opt_in() {
    let reporter = Reporter::builder()
        .application("telemetry-test")
        .build()
        .unwrap();

    let diagnostic = reporter.warning("explicit message").label(
        "/home/alice/private.rs",
        9,
        Some(2),
        Some(4),
        Some("label detail"),
    );

    let policy = TelemetryPolicy::default()
        .with_message(TextExport::Plaintext)
        .with_label_messages(TextExport::Plaintext)
        .with_locations(LocationExport::FileName);

    let event = TelemetryAdapter::new(policy).event(&diagnostic);

    assert_eq!(
        string_attribute(&event, "diagprint.message",),
        Some("explicit message".to_owned())
    );

    assert_eq!(
        string_attribute(&event, "diagprint.source.file",),
        Some("private.rs".to_owned())
    );

    assert_eq!(
        string_attribute(&event, "diagprint.source.message",),
        Some("label detail".to_owned())
    );

    assert!(!event.marks_span_error());
}

#[test]
fn redacted_optional_fields_never_reveal_original_values() {
    let reporter = Reporter::builder()
        .application("telemetry-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("message-secret")
        .help("help-secret")
        .note("note-secret")
        .cause("cause-secret");

    let policy = TelemetryPolicy::default()
        .with_help(TextExport::Redact)
        .with_notes(TextExport::Redact)
        .with_causes(TextExport::Redact);

    let event = TelemetryAdapter::new(policy).event(&diagnostic);

    for key in [
        "diagprint.message",
        "diagprint.help",
        "diagprint.note.0",
        "diagprint.cause.0",
    ] {
        assert_eq!(string_attribute(&event, key,), Some(REDACTED.to_owned()));
    }
}

#[test]
fn metadata_remains_available_under_default_redaction() {
    let reporter = Reporter::builder()
        .application("telemetry-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("secret text").code("E-TELEMETRY");

    let event = TelemetryAdapter::default().event(&diagnostic);

    assert_eq!(
        string_attribute(&event, "diagprint.code",),
        Some("E-TELEMETRY".to_owned())
    );

    assert_eq!(
        string_attribute(&event, "diagprint.severity",),
        Some("error".to_owned())
    );

    assert_eq!(
        string_attribute(&event, "diagprint.application",),
        Some("telemetry-test".to_owned())
    );

    assert_eq!(
        string_attribute(&event, "diagprint.message",),
        Some(REDACTED.to_owned())
    );
}
