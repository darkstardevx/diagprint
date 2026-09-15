use super::Renderer;
use crate::{Diagnostic, Severity};
use std::fs;
use terminal_size::{terminal_size, Width};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MIN_WIDTH: usize = 40;
const DEFAULT_WIDTH: usize = 72;
const TAB_WIDTH: usize = 4;

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
            width: DEFAULT_WIDTH,
        }
    }
}

#[derive(Debug)]
struct SourceWindow {
    text: String,
    caret_offset: usize,
    caret_width: usize,
}

fn strip_ansi(s: &str) -> String {
    let mut output = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();

            for ansi_ch in chars.by_ref() {
                if ansi_ch.is_ascii_alphabetic() {
                    break;
                }
            }

            continue;
        }

        output.push(ch);
    }

    output
}

fn visible_len(s: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(s).as_str())
}

fn truncate_visible(s: &str, max_width: usize) -> String {
    if visible_len(s) <= max_width {
        return s.to_owned();
    }

    let mut output = String::new();
    let mut width = 0;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            output.push(ch);
            output.push(chars.next().expect("ANSI sequence prefix disappeared"));

            for ansi_ch in chars.by_ref() {
                output.push(ansi_ch);

                if ansi_ch.is_ascii_alphabetic() {
                    break;
                }
            }

            continue;
        }

        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);

        if width + char_width > max_width {
            break;
        }

        output.push(ch);
        width += char_width;
    }

    output
}

fn wrap_visible(s: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }

    if s.is_empty() {
        return vec![String::new()];
    }

    let mut output = Vec::new();

    for logical_line in s.lines() {
        if logical_line.is_empty() {
            output.push(String::new());
            continue;
        }

        let mut current = String::new();
        let mut current_width = 0;

        for word in logical_line.split_whitespace() {
            let word_width = UnicodeWidthStr::width(word);

            if current.is_empty() {
                if word_width <= max_width {
                    current.push_str(word);
                    current_width = word_width;
                } else {
                    let mut fragment = String::new();
                    let mut fragment_width = 0;

                    for ch in word.chars() {
                        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);

                        if fragment_width + char_width > max_width && !fragment.is_empty() {
                            output.push(fragment);
                            fragment = String::new();
                            fragment_width = 0;
                        }

                        fragment.push(ch);
                        fragment_width += char_width;
                    }

                    current = fragment;
                    current_width = fragment_width;
                }

                continue;
            }

            if current_width + 1 + word_width <= max_width {
                current.push(' ');
                current.push_str(word);
                current_width += 1 + word_width;
            } else {
                output.push(current);
                current = String::new();

                if word_width <= max_width {
                    current.push_str(word);
                    current_width = word_width;
                } else {
                    let mut fragment = String::new();
                    let mut fragment_width = 0;

                    for ch in word.chars() {
                        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);

                        if fragment_width + char_width > max_width && !fragment.is_empty() {
                            output.push(fragment);
                            fragment = String::new();
                            fragment_width = 0;
                        }

                        fragment.push(ch);
                        fragment_width += char_width;
                    }

                    current = fragment;
                    current_width = fragment_width;
                }
            }
        }

        if !current.is_empty() {
            output.push(current);
        }
    }

    if output.is_empty() {
        output.push(String::new());
    }

    output
}

fn row(s: &str, width: usize) -> String {
    let content_width = width.saturating_sub(4);
    let text = truncate_visible(s, content_width);
    let padding = content_width.saturating_sub(visible_len(&text));

    format!("│ {text}{} │\n", " ".repeat(padding))
}

fn wrapped_rows(s: &str, width: usize) -> String {
    let content_width = width.saturating_sub(4);
    let mut output = String::new();

    for line in wrap_visible(s, content_width) {
        output.push_str(&row(&line, width));
    }

    output
}

fn prefixed_rows(prefix: &str, text: &str, width: usize) -> String {
    let content_width = width.saturating_sub(4);
    let prefix_width = visible_len(prefix);
    let available = content_width.saturating_sub(prefix_width).max(1);

    let lines = wrap_visible(text, available);
    let mut output = String::new();

    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            output.push_str(&row(&format!("{prefix}{line}"), width));
        } else {
            output.push_str(&row(
                &format!("{}{}", " ".repeat(prefix_width), line),
                width,
            ));
        }
    }

    output
}

fn expand_source_line(line: &str) -> (String, Vec<usize>) {
    let mut rendered = String::new();
    let mut offsets = Vec::new();
    let mut width = 0;

    for ch in line.chars() {
        offsets.push(width);

        if ch == '\t' {
            let spaces = TAB_WIDTH - (width % TAB_WIDTH);
            rendered.push_str(&" ".repeat(spaces));
            width += spaces;
        } else {
            rendered.push(ch);
            width += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
    }

    offsets.push(width);

    (rendered, offsets)
}

fn slice_display_width(s: &str, start: usize, max_width: usize) -> (String, usize) {
    let mut output = String::new();
    let mut position = 0;
    let mut used = 0;
    let mut actual_start = None;

    for ch in s.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        let next_position = position + char_width;

        if next_position <= start {
            position = next_position;
            continue;
        }

        if actual_start.is_none() {
            actual_start = Some(position);
        }

        if used + char_width > max_width {
            break;
        }

        output.push(ch);
        used += char_width;
        position = next_position;
    }

    (output, actual_start.unwrap_or(start))
}

fn source_window(
    line: &str,
    column: usize,
    highlight_length: usize,
    max_width: usize,
) -> SourceWindow {
    if max_width == 0 {
        return SourceWindow {
            text: String::new(),
            caret_offset: 0,
            caret_width: 1,
        };
    }

    let (rendered, offsets) = expand_source_line(line);

    let source_char_count = offsets.len().saturating_sub(1);
    let target_index = column.saturating_sub(1).min(source_char_count);

    let highlight_end_index = target_index
        .saturating_add(highlight_length.max(1))
        .min(source_char_count);

    let focus_start = offsets[target_index];

    let focus_end = if highlight_end_index > target_index {
        offsets[highlight_end_index]
    } else {
        focus_start.saturating_add(1)
    };

    let highlight_width = focus_end.saturating_sub(focus_start).max(1);
    let total_width = visible_len(&rendered);

    if total_width <= max_width {
        return SourceWindow {
            text: rendered,
            caret_offset: focus_start,
            caret_width: highlight_width,
        };
    }

    let window_budget = max_width.saturating_sub(2).max(1);
    let mut requested_start = focus_start.saturating_sub(window_budget / 3);

    if requested_start + window_budget > total_width {
        requested_start = total_width.saturating_sub(window_budget);
    }

    let (segment, actual_start) = slice_display_width(&rendered, requested_start, window_budget);

    let segment_width = visible_len(&segment);
    let left_clipped = actual_start > 0;
    let right_clipped = actual_start + segment_width < total_width;

    let mut text = String::new();

    if left_clipped {
        text.push('…');
    }

    text.push_str(&segment);

    if right_clipped {
        text.push('…');
    }

    let left_marker_width = usize::from(left_clipped);

    let caret_offset = focus_start
        .saturating_sub(actual_start)
        .saturating_add(left_marker_width);

    let available_highlight = max_width.saturating_sub(caret_offset).max(1);

    SourceWindow {
        text,
        caret_offset,
        caret_width: highlight_width.min(available_highlight).max(1),
    }
}

impl TerminalRenderer {
    fn effective_width(&self) -> usize {
        if self.width != DEFAULT_WIDTH {
            return self.width.max(MIN_WIDTH);
        }

        terminal_size()
            .map(|(Width(width), _)| usize::from(width))
            .unwrap_or(DEFAULT_WIDTH)
            .max(MIN_WIDTH)
    }

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
                .map(|code| format!(" [{code}]"))
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

    fn source(&self, d: &Diagnostic, terminal_width: usize) -> Vec<String> {
        let mut output = Vec::new();
        let content_width = terminal_width.saturating_sub(4);

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

                if target >= lines.len() {
                    continue;
                }

                let start = target.saturating_sub(self.source_context_lines);
                let end = (target + self.source_context_lines + 1).min(lines.len());
                let gutter_width = end.to_string().len();

                let source_prefix_width = gutter_width + 5;
                let source_width = content_width.saturating_sub(source_prefix_width);

                let column = location.column.unwrap_or(1) as usize;
                let highlight_length = label.length.unwrap_or(1).max(1);

                for (index, line) in lines.iter().enumerate().take(end).skip(start) {
                    let window = source_window(line, column, highlight_length, source_width);

                    let target_marker = if index == target { ">" } else { " " };

                    output.push(format!(
                        "{target_marker} {:>width$} │ {}",
                        index + 1,
                        window.text,
                        width = gutter_width
                    ));

                    if index != target {
                        continue;
                    }

                    let annotation_gutter = format!("  {:>width$} │ ", "", width = gutter_width);

                    let caret_indent = " ".repeat(window.caret_offset);
                    let carets = "^".repeat(window.caret_width);

                    output.push(format!("{annotation_gutter}{caret_indent}{carets}"));

                    if let Some(message) = &label.message {
                        let label_marker = "└─ ";

                        let label_prefix =
                            format!("{annotation_gutter}{caret_indent}{label_marker}");

                        let label_prefix_width = visible_len(&label_prefix);

                        let available = content_width.saturating_sub(label_prefix_width).max(1);

                        let wrapped = wrap_visible(message, available);

                        for (message_index, message_line) in wrapped.iter().enumerate() {
                            if message_index == 0 {
                                output.push(format!("{label_prefix}{message_line}"));
                            } else {
                                output.push(format!(
                                    "{}{}",
                                    " ".repeat(label_prefix_width),
                                    message_line
                                ));
                            }
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
        let width = self.effective_width();
        let title = self.title(d);
        let title_width = visible_len(&title);

        let mut output = format!(
            "╭─ {title} {}╮\n",
            "─".repeat(width.saturating_sub(title_width + 5))
        );

        output.push_str(&wrapped_rows(&d.message, width));

        if !d.labels.is_empty() {
            output.push_str(&row("", width));

            for source_line in self.source(d, width) {
                output.push_str(&row(&source_line, width));
            }
        }

        if let Some(cause) = &d.cause {
            output.push_str(&row("", width));
            output.push_str(&row("CAUSE", width));

            for (depth, cause) in cause.iter().enumerate() {
                let prefix = format!("{}└─ ", "   ".repeat(depth));
                output.push_str(&prefixed_rows(&prefix, &cause.message, width));
            }
        }

        if !d.notes.is_empty() {
            output.push_str(&row("", width));

            for note in &d.notes {
                output.push_str(&prefixed_rows("NOTE  ", note, width));
            }
        }

        if let Some(help) = &d.help {
            output.push_str(&row("", width));
            output.push_str(&prefixed_rows("HELP  ", help, width));
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
                output.push_str(&wrapped_rows(&metadata, width));
            }
        }

        output.push_str(&format!("╰{}╯\n", "─".repeat(width.saturating_sub(2))));

        output
    }
}
