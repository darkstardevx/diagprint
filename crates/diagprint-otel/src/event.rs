use crate::{AttributeExport, LocationExport, TelemetryPolicy, TextExport};
use diagprint::{
    Diagnostic, DiagnosticAttribute, DiagnosticValue, Label, LabelKind, REDACTED, Severity,
};
use opentelemetry::KeyValue;

pub const DIAGNOSTIC_EVENT_NAME: &str = "diagprint.diagnostic";

#[derive(Debug, Clone)]
pub struct TelemetryEvent {
    attributes: Vec<KeyValue>,
    mark_span_error: bool,
}

impl TelemetryEvent {
    pub fn attributes(&self) -> &[KeyValue] {
        &self.attributes
    }

    pub fn into_attributes(self) -> Vec<KeyValue> {
        self.attributes
    }

    pub const fn marks_span_error(&self) -> bool {
        self.mark_span_error
    }
}

pub(crate) fn build_event(diagnostic: &Diagnostic, policy: TelemetryPolicy) -> TelemetryEvent {
    let mut attributes = Vec::with_capacity(25 + diagnostic.attributes.len());

    attributes.push(KeyValue::new(
        "diagprint.report_id",
        diagnostic.report_id.to_string(),
    ));

    attributes.push(KeyValue::new(
        "diagprint.session_id",
        diagnostic.session_id.to_string(),
    ));

    attributes.push(KeyValue::new(
        "diagprint.timestamp",
        diagnostic.timestamp.to_rfc3339(),
    ));

    attributes.push(KeyValue::new(
        "diagprint.severity",
        severity_name(diagnostic.severity),
    ));

    if policy.include_application {
        attributes.push(KeyValue::new(
            "diagprint.application",
            diagnostic.application.clone(),
        ));
    }

    if policy.include_hostname {
        attributes.push(KeyValue::new("host.name", diagnostic.hostname.clone()));
    }

    if policy.include_process_id {
        attributes.push(KeyValue::new("process.pid", i64::from(diagnostic.pid)));
    }

    if let Some(code) = &diagnostic.code {
        attributes.push(KeyValue::new("diagprint.code", code.clone()));
    }

    push_text(
        &mut attributes,
        "diagprint.message",
        policy.message,
        &diagnostic.message,
    );

    if let Some(help) = &diagnostic.help {
        push_text(&mut attributes, "diagprint.help", policy.help, help);
    }

    for (index, note) in diagnostic.notes.iter().enumerate() {
        push_text(
            &mut attributes,
            format!("diagprint.note.{index}"),
            policy.notes,
            note,
        );
    }

    if let Some(cause) = &diagnostic.cause {
        for (index, cause) in cause.iter().enumerate() {
            push_text(
                &mut attributes,
                format!("diagprint.cause.{index}"),
                policy.causes,
                &cause.message,
            );
        }
    }

    attributes.push(KeyValue::new(
        "diagprint.label_count",
        usize_to_i64(diagnostic.labels.len()),
    ));

    attributes.push(KeyValue::new(
        "diagprint.note_count",
        usize_to_i64(diagnostic.notes.len()),
    ));

    attributes.push(KeyValue::new(
        "diagprint.attribute_count",
        usize_to_i64(diagnostic.attributes.len()),
    ));

    attributes.push(KeyValue::new(
        "diagprint.suggestion_count",
        usize_to_i64(diagnostic.suggestions.len()),
    ));

    attributes.push(KeyValue::new(
        "diagprint.has_machine_fix",
        diagnostic
            .suggestions
            .iter()
            .any(|suggestion| suggestion.is_machine_applicable() && suggestion.has_edits()),
    ));

    push_diagnostic_attributes(&mut attributes, &diagnostic.attributes, policy.attributes);

    if let Some(label) = primary_label(diagnostic) {
        push_location(
            &mut attributes,
            label,
            policy.locations,
            policy.label_messages,
        );
    }

    TelemetryEvent {
        attributes,

        mark_span_error: policy.mark_error_status && diagnostic.severity >= Severity::Error,
    }
}

fn push_diagnostic_attributes(
    output: &mut Vec<KeyValue>,
    diagnostic_attributes: &[DiagnosticAttribute],
    policy: AttributeExport,
) {
    match policy {
        AttributeExport::Omit => {}

        AttributeExport::Redact => {
            for attribute in diagnostic_attributes {
                output.push(KeyValue::new(attribute_key(&attribute.name), REDACTED));
            }
        }

        AttributeExport::Full => {
            for attribute in diagnostic_attributes {
                push_full_attribute(output, attribute);
            }
        }
    }
}

fn push_full_attribute(output: &mut Vec<KeyValue>, attribute: &DiagnosticAttribute) {
    let key = attribute_key(&attribute.name);

    let value = &attribute.value;

    let key_value = match value {
        DiagnosticValue::String(value) => KeyValue::new(key, value.clone()),

        DiagnosticValue::Bool(value) => KeyValue::new(key, *value),

        DiagnosticValue::I64(value) => KeyValue::new(key, *value),

        DiagnosticValue::U64(value) => match i64::try_from(*value) {
            Ok(value) => KeyValue::new(key, value),

            Err(_) => KeyValue::new(key, value.to_string()),
        },

        DiagnosticValue::I128(value) => match i64::try_from(*value) {
            Ok(value) => KeyValue::new(key, value),

            Err(_) => KeyValue::new(key, value.to_string()),
        },

        DiagnosticValue::U128(value) => match i64::try_from(*value) {
            Ok(value) => KeyValue::new(key, value),

            Err(_) => KeyValue::new(key, value.to_string()),
        },

        DiagnosticValue::F64(value) => KeyValue::new(key, *value),
    };

    output.push(key_value);
}

fn attribute_key(name: &str) -> String {
    format!("diagprint.attribute.{name}")
}

fn push_location(
    attributes: &mut Vec<KeyValue>,
    label: &Label,
    location_policy: LocationExport,
    message_policy: TextExport,
) {
    let Some(file) = location_policy.apply(&label.location.file) else {
        return;
    };

    attributes.push(KeyValue::new("diagprint.source.file", file));

    attributes.push(KeyValue::new(
        "diagprint.source.line",
        i64::from(label.location.line),
    ));

    if let Some(column) = label.location.column {
        attributes.push(KeyValue::new("diagprint.source.column", i64::from(column)));
    }

    if let Some(revision) = label.location.revision {
        attributes.push(KeyValue::new(
            "diagprint.source.revision",
            u64_to_i64(revision.get()),
        ));
    }

    if let Some(message) = &label.message {
        push_text(
            attributes,
            "diagprint.source.message",
            message_policy,
            message,
        );
    }
}

fn push_text(
    attributes: &mut Vec<KeyValue>,
    key: impl Into<String>,
    policy: TextExport,
    value: &str,
) {
    if let Some(value) = policy.apply(value) {
        attributes.push(KeyValue::new(key.into(), value));
    }
}

fn primary_label(diagnostic: &Diagnostic) -> Option<&Label> {
    diagnostic
        .labels
        .iter()
        .find(|label| label.kind == LabelKind::Primary)
        .or_else(|| diagnostic.labels.first())
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Trace => "trace",
        Severity::Debug => "debug",
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "fatal",
    }
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
