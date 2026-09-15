use super::Renderer;
use crate::{Diagnostic, Label, LabelKind};

#[derive(Debug, Default, Clone, Copy)]
pub struct PlainRenderer;

fn label_kind_name(kind: LabelKind) -> &'static str {
    match kind {
        LabelKind::Primary => "primary",
        LabelKind::Secondary => "secondary",
    }
}

fn label_text(label: &Label) -> String {
    let location = &label.location;

    let mut output = format!(
        "{}:{}{}",
        location.file,
        location.line,
        location
            .column
            .map(|column| format!(":{column}"))
            .unwrap_or_default()
    );

    if let Some(length) = label.length {
        output.push_str(&format!(" length={length}"));
    }

    if let Some(message) = &label.message {
        output.push_str(" — ");
        output.push_str(message);
    }

    output
}

impl Renderer for PlainRenderer {
    fn render(&self, d: &Diagnostic) -> String {
        let mut o = format!(
            "{} app={} pid={} host={} session={} report={} {}{}: {}\n",
            d.timestamp.to_rfc3339(),
            d.application,
            d.pid,
            d.hostname,
            d.session_id,
            d.report_id,
            d.severity,
            d.code
                .as_ref()
                .map(|c| format!(" [{c}]"))
                .unwrap_or_default(),
            d.message
        );

        if !d.labels.is_empty() {
            o.push_str("  labels:\n");

            for label in &d.labels {
                o.push_str(&format!(
                    "    {}: {}\n",
                    label_kind_name(label.kind),
                    label_text(label)
                ));
            }
        }

        if let Some(c) = &d.cause {
            for (i, x) in c.iter().enumerate() {
                o.push_str(&format!("  {}└─ {}\n", "  ".repeat(i), x.message));
            }
        }

        for n in &d.notes {
            o.push_str(&format!("  note: {n}\n"));
        }

        if let Some(h) = &d.help {
            o.push_str(&format!("  help: {h}\n"));
        }

        o
    }
}
