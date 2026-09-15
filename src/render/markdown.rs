use super::Renderer;
use crate::{Diagnostic, Label, LabelKind};

#[derive(Debug, Default, Clone, Copy)]
pub struct MarkdownRenderer;

fn label_kind_name(kind: LabelKind) -> &'static str {
    match kind {
        LabelKind::Primary => "Primary",
        LabelKind::Secondary => "Secondary",
    }
}

fn label_location(label: &Label) -> String {
    let location = &label.location;

    format!(
        "{}:{}{}",
        location.file,
        location.line,
        location
            .column
            .map(|column| format!(":{column}"))
            .unwrap_or_default()
    )
}

impl Renderer for MarkdownRenderer {
    fn render(&self, d: &Diagnostic) -> String {
        let mut o = format!(
            "# {}{}\n\n**Application:** `{}`  \n**Timestamp:** `{}`  \n**PID:** `{}`  \n**Host:** `{}`  \n**Session:** `{}`  \n**Report:** `{}`\n\n## Message\n\n{}\n",
            d.severity,
            d.code
                .as_ref()
                .map(|c| format!(" — `{c}`"))
                .unwrap_or_default(),
            d.application,
            d.timestamp.to_rfc3339(),
            d.pid,
            d.hostname,
            d.session_id,
            d.report_id,
            d.message
        );

        if !d.labels.is_empty() {
            o.push_str("\n## Labels\n\n");

            for label in &d.labels {
                o.push_str(&format!(
                    "- **{}:** `{}`",
                    label_kind_name(label.kind),
                    label_location(label)
                ));

                if let Some(length) = label.length {
                    o.push_str(&format!(" (length: `{length}`)"));
                }

                if let Some(message) = &label.message {
                    o.push_str(" — ");
                    o.push_str(message);
                }

                o.push('\n');
            }
        }

        if let Some(c) = &d.cause {
            o.push_str("\n## Causes\n\n");

            for (i, x) in c.iter().enumerate() {
                o.push_str(&format!("{}- {}\n", "  ".repeat(i), x.message));
            }
        }

        if !d.notes.is_empty() {
            o.push_str("\n## Notes\n\n");

            for n in &d.notes {
                o.push_str(&format!("- {n}\n"));
            }
        }

        if let Some(h) = &d.help {
            o.push_str(&format!("\n## Help\n\n{h}\n"));
        }

        o
    }
}
