use super::Renderer;
use crate::{Diagnostic, Severity};
use std::fs;

#[derive(Debug, Clone)]
pub struct TerminalRenderer {
    pub color: bool,
    pub show_metadata: bool,
    pub source_context_lines: usize,
    pub width: usize,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self {
            color: true,
            show_metadata: false,
            source_context_lines: 1,
            width: 72,
        }
    }
}

fn visible_len(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;

    for c in s.chars() {
        if c == '\x1b' {
            esc = true;
        } else if esc && c == 'm' {
            esc = false;
        } else if !esc {
            n += 1;
        }
    }

    n
}

fn row(s: &str, width: usize) -> String {
    let max = width.saturating_sub(4);
    let text: String = s.chars().take(max).collect();
    let pad = max.saturating_sub(visible_len(&text));

    format!("│ {text}{} │\n", " ".repeat(pad))
}

impl TerminalRenderer {
    fn title(&self, d: &Diagnostic) -> String {
        let icon = match d.severity {
            Severity::Trace => "·",
            Severity::Debug => "◆",
            Severity::Info => "ℹ",
            Severity::Warning => "⚠",
            Severity::Error => "✖",
            Severity::Fatal => "☠",
        };

        let raw = format!(
            "{icon} {}{}",
            d.severity,
            d.code
                .as_ref()
                .map(|c| format!(" [{c}]"))
                .unwrap_or_default()
        );

        if !self.color {
            return raw;
        }

        let ansi = match d.severity {
            Severity::Trace | Severity::Debug => "\x1b[90m",
            Severity::Info => "\x1b[36m",
            Severity::Warning => "\x1b[33m",
            Severity::Error => "\x1b[31m",
            Severity::Fatal => "\x1b[35;1m",
        };

        format!("{ansi}{raw}\x1b[0m")
    }

    fn source(&self, d: &Diagnostic) -> Vec<String> {
        let mut output = Vec::new();

        for label in &d.labels {
            let location = &label.location;

            output.push(format!(
                "--> {}:{}{}",
                location.file,
                location.line,
                location
                    .column
                    .map(|column| format!(":{column}"))
                    .unwrap_or_default()
            ));

            if let Ok(source) = fs::read_to_string(&location.file) {
                let lines: Vec<_> = source.lines().collect();
                let target = location.line.saturating_sub(1) as usize;

                if target < lines.len() {
                    let start = target.saturating_sub(self.source_context_lines);
                    let end = (target + self.source_context_lines + 1).min(lines.len());
                    let gutter_width = end.to_string().len();

                    for (index, line) in lines.iter().enumerate().take(end).skip(start) {
                        output.push(format!(
                            "{:>width$} │ {}",
                            index + 1,
                            line,
                            width = gutter_width
                        ));

                        if index == target {
                            let column = location.column.unwrap_or(1).saturating_sub(1) as usize;
                            let length = label.length.unwrap_or(1).max(1);

                            output.push(format!(
                                "{:>width$} │ {}{}{}",
                                " ",
                                " ".repeat(column),
                                "^".repeat(length),
                                label
                                    .message
                                    .as_ref()
                                    .map(|message| format!(" {message}"))
                                    .unwrap_or_default(),
                                width = gutter_width
                            ));
                        }
                    }
                }
            }
        }

        output
    }
}

impl Renderer for TerminalRenderer {
    fn render(&self, d: &Diagnostic) -> String {
        let width = self.width.max(40);
        let title = self.title(d);
        let title_width = visible_len(&title);

        let mut output = format!(
            "╭─ {title} {}╮\n",
            "─".repeat(width.saturating_sub(title_width + 5))
        );

        output.push_str(&row(&d.message, width));

        for source_line in self.source(d) {
            output.push_str(&row(&source_line, width));
        }

        if let Some(cause) = &d.cause {
            output.push_str(&row("", width));
            output.push_str(&row("Caused by", width));

            for (depth, cause) in cause.iter().enumerate() {
                output.push_str(&row(
                    &format!("{}└─ {}", "   ".repeat(depth), cause.message),
                    width,
                ));
            }
        }

        for note in &d.notes {
            output.push_str(&row("", width));
            output.push_str(&row(&format!("NOTE  {note}"), width));
        }

        if let Some(help) = &d.help {
            output.push_str(&row(&format!("HELP  {help}"), width));
        }

        if self.show_metadata {
            output.push_str(&format!(
                "├─ Diagnostic {}┤\n",
                "─".repeat(width.saturating_sub(15))
            ));

            for metadata in [
                format!("Timestamp {}", d.timestamp.to_rfc3339()),
                format!("App       {}", d.application),
                format!("PID       {}", d.pid),
                format!("Host      {}", d.hostname),
                format!("Session   {}", d.session_id),
                format!("Report    {}", d.report_id),
            ] {
                output.push_str(&row(&metadata, width));
            }
        }

        output.push_str(&format!("╰{}╯\n", "─".repeat(width.saturating_sub(2))));

        output
    }
}
