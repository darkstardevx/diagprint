use super::{Renderer, Style, Theme};
use crate::{
    CapturedDiagnostic, Diagnostic, LabelKind, Severity, SourceCache, SourceRevision,
    SourceSnapshot, Suggestion,
};
use std::{fs, sync::Arc};
use terminal_size::{Width, terminal_size};
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
    pub theme: Theme,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self {
            color: true,
            show_metadata: false,
            source_context_lines: 1,
            width: DEFAULT_WIDTH,
            theme: Theme::default(),
        }
    }
}

#[derive(Debug)]
struct SourceWindow {
    text: String,
    caret_offset: usize,
    caret_width: usize,
}

enum SourceText {
    Cached(Arc<str>),
    File(String),
}

impl SourceText {
    fn as_str(&self) -> &str {
        match self {
            Self::Cached(source) => source,
            Self::File(source) => source,
        }
    }
}

#[derive(Clone, Copy)]
enum SourceStore<'a> {
    Cache(&'a SourceCache),
    Snapshot(&'a SourceSnapshot),
}

impl SourceStore<'_> {
    fn get(self, name: &str) -> Option<Arc<str>> {
        match self {
            Self::Cache(cache) => cache.get(name),
            Self::Snapshot(snapshot) => snapshot.get(name),
        }
    }

    fn revision(self, name: &str) -> Option<SourceRevision> {
        match self {
            Self::Cache(cache) => cache.revision(name),
            Self::Snapshot(snapshot) => snapshot.revision(name),
        }
    }
}

fn load_source_text(sources: Option<SourceStore<'_>>, name: &str) -> Option<SourceText> {
    if let Some(sources) = sources {
        if let Some(source) = sources.get(name) {
            return Some(SourceText::Cached(source));
        }
    }

    fs::read_to_string(name).ok().map(SourceText::File)
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
    let contains_ansi = s.contains('\x1b');

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

    if contains_ansi {
        output.push_str("\x1b[0m");
    }

    output
}

fn wrap_visible(s: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || s.is_empty() {
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

fn hard_wrap_visible(s: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || s.is_empty() {
        return vec![String::new()];
    }

    let mut output = Vec::new();

    for logical_line in s.lines() {
        let mut current = String::new();
        let mut current_width = 0;

        for ch in logical_line.chars() {
            let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);

            if current_width + char_width > max_width && !current.is_empty() {
                output.push(current);
                current = String::new();
                current_width = 0;
            }

            current.push(ch);
            current_width += char_width;
        }

        output.push(current);
    }

    if output.is_empty() {
        output.push(String::new());
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

    let caret_offset = focus_start
        .saturating_sub(actual_start)
        .saturating_add(usize::from(left_clipped));

    let available_highlight = max_width.saturating_sub(caret_offset).max(1);

    SourceWindow {
        text,
        caret_offset,
        caret_width: highlight_width.min(available_highlight).max(1),
    }
}

impl TerminalRenderer {
    fn paint(&self, style: &Style, text: &str) -> String {
        style.paint(self.color, text)
    }

    fn effective_width(&self) -> usize {
        if self.width != DEFAULT_WIDTH {
            return self.width.max(MIN_WIDTH);
        }

        terminal_size()
            .map(|(Width(width), _)| usize::from(width))
            .unwrap_or(DEFAULT_WIDTH)
            .max(MIN_WIDTH)
    }

    fn row(&self, text: &str, width: usize) -> String {
        let content_width = width.saturating_sub(4);
        let text = truncate_visible(text, content_width);
        let padding = content_width.saturating_sub(visible_len(&text));

        format!(
            "{} {text}{} {}\n",
            self.paint(&self.theme.border, "│"),
            " ".repeat(padding),
            self.paint(&self.theme.border, "│")
        )
    }

    fn wrapped_rows(&self, text: &str, style: &Style, width: usize) -> String {
        let content_width = width.saturating_sub(4);
        let mut output = String::new();

        for line in wrap_visible(text, content_width) {
            output.push_str(&self.row(&self.paint(style, &line), width));
        }

        output
    }

    fn prefixed_rows(
        &self,
        prefix: &str,
        prefix_style: &Style,
        text: &str,
        text_style: &Style,
        width: usize,
    ) -> String {
        let content_width = width.saturating_sub(4);
        let prefix_width = visible_len(prefix);
        let available = content_width.saturating_sub(prefix_width).max(1);

        let lines = wrap_visible(text, available);
        let mut output = String::new();

        for (index, line) in lines.iter().enumerate() {
            let painted_line = self.paint(text_style, line);

            if index == 0 {
                output.push_str(&self.row(
                    &format!("{}{}", self.paint(prefix_style, prefix), painted_line),
                    width,
                ));
            } else {
                output.push_str(&self.row(
                    &format!("{}{}", " ".repeat(prefix_width), painted_line),
                    width,
                ));
            }
        }

        output
    }

    fn hard_prefixed_rows(
        &self,
        prefix: &str,
        prefix_style: &Style,
        text: &str,
        text_style: &Style,
        width: usize,
    ) -> String {
        let content_width = width.saturating_sub(4);
        let prefix_width = visible_len(prefix);
        let available = content_width.saturating_sub(prefix_width).max(1);

        let lines = hard_wrap_visible(text, available);
        let mut output = String::new();

        for (index, line) in lines.iter().enumerate() {
            let painted_line = self.paint(text_style, line);

            if index == 0 {
                output.push_str(&self.row(
                    &format!("{}{}", self.paint(prefix_style, prefix), painted_line),
                    width,
                ));
            } else {
                output.push_str(&self.row(
                    &format!("{}{}", " ".repeat(prefix_width), painted_line),
                    width,
                ));
            }
        }

        output
    }

    fn source(
        &self,
        diagnostic: &Diagnostic,
        terminal_width: usize,
        sources: Option<SourceStore<'_>>,
    ) -> Vec<String> {
        let mut output = Vec::new();
        let content_width = terminal_width.saturating_sub(4);

        for label in &diagnostic.labels {
            let location = &label.location;
            let primary = label.kind == LabelKind::Primary;

            let kind_name = if primary { "primary" } else { "secondary" };
            let location_marker = if primary { "-->" } else { ":::" };
            let target_marker = if primary { ">" } else { ":" };
            let caret_marker = if primary { "^" } else { "-" };
            let label_marker = if primary { "└─ " } else { "└· " };

            let path_style = if primary {
                &self.theme.source_path
            } else {
                &self.theme.source_gutter
            };

            let target_style = if primary {
                &self.theme.source_target
            } else {
                &self.theme.source_gutter
            };

            let caret_style = if primary {
                &self.theme.source_caret
            } else {
                &self.theme.source_gutter
            };

            let label_style = if primary {
                &self.theme.source_label
            } else {
                &self.theme.source_gutter
            };

            let location_text = format!(
                "{location_marker} {kind_name} {}:{}{}",
                location.file,
                location.line,
                location
                    .column
                    .map(|column| format!(":{column}"))
                    .unwrap_or_default()
            );

            output.push(self.paint(path_style, &location_text));

            if let Some(expected_revision) = location.revision {
                match sources.and_then(|source| source.revision(&location.file)) {
                    Some(actual_revision) if actual_revision == expected_revision => {}

                    Some(actual_revision) => {
                        output.push(self.paint(
                            &self.theme.source_gutter,
                            &format!("! stale source: r{expected_revision} != r{actual_revision}"),
                        ));

                        continue;
                    }

                    None => {
                        output.push(self.paint(
                            &self.theme.source_gutter,
                            &format!(
                                "! source revision unavailable: expected                                  r{expected_revision}"
                            ),
                        ));

                        continue;
                    }
                }
            }

            if let Some(source) = load_source_text(sources, &location.file) {
                let lines: Vec<_> = source.as_str().lines().collect();
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

                    let marker = if index == target {
                        self.paint(target_style, target_marker)
                    } else {
                        " ".into()
                    };

                    let line_number = self.paint(
                        &self.theme.source_gutter,
                        &format!("{:>width$}", index + 1, width = gutter_width),
                    );

                    let gutter = self.paint(&self.theme.source_gutter, "│");

                    output.push(format!("{marker} {line_number} {gutter} {}", window.text));

                    if index != target {
                        continue;
                    }

                    let annotation_gutter = format!(
                        "  {} {} ",
                        " ".repeat(gutter_width),
                        self.paint(&self.theme.source_gutter, "│")
                    );

                    let caret_indent = " ".repeat(window.caret_offset);

                    let carets = self.paint(caret_style, &caret_marker.repeat(window.caret_width));

                    output.push(format!("{annotation_gutter}{caret_indent}{carets}"));

                    if let Some(message) = &label.message {
                        let painted_label_marker = self.paint(label_style, label_marker);

                        let plain_prefix_width = visible_len(&annotation_gutter)
                            + window.caret_offset
                            + visible_len(label_marker);

                        let available = content_width.saturating_sub(plain_prefix_width).max(1);

                        let wrapped = wrap_visible(message, available);

                        for (message_index, message_line) in wrapped.iter().enumerate() {
                            if message_index == 0 {
                                output.push(format!(
                                    "{annotation_gutter}{caret_indent}{}{}",
                                    painted_label_marker,
                                    self.paint(label_style, message_line)
                                ));
                            } else {
                                output.push(format!(
                                    "{}{}",
                                    " ".repeat(plain_prefix_width),
                                    self.paint(label_style, message_line)
                                ));
                            }
                        }
                    }
                }
            }
        }

        output
    }

    fn suggestion_rows(&self, suggestion: &Suggestion, width: usize) -> String {
        let mut output = String::new();

        output.push_str(&self.row("", width));
        output.push_str(&self.row(&self.paint(&self.theme.suggestion, "SUGGESTION"), width));

        output.push_str(&self.prefixed_rows(
            "TITLE  ",
            &self.theme.suggestion,
            &suggestion.title,
            &self.theme.message,
            width,
        ));

        if let Some(explanation) = &suggestion.explanation {
            output.push_str(&self.prefixed_rows(
                "WHY    ",
                &self.theme.applicability,
                explanation,
                &self.theme.message,
                width,
            ));
        }

        for edit in &suggestion.edits {
            for line in edit.preview_lines() {
                if let Some(path) = line.strip_prefix("PATCH ") {
                    output.push_str(&self.hard_prefixed_rows(
                        "PATCH  ",
                        &self.theme.suggestion,
                        path,
                        &self.theme.metadata_value,
                        width,
                    ));
                } else if let Some(added) = line.strip_prefix("+ ") {
                    output.push_str(&self.hard_prefixed_rows(
                        "+ ",
                        &self.theme.patch_add,
                        added,
                        &self.theme.patch_add,
                        width,
                    ));
                } else if let Some(removed) = line.strip_prefix("- ") {
                    output.push_str(&self.hard_prefixed_rows(
                        "- ",
                        &self.theme.patch_remove,
                        removed,
                        &self.theme.patch_remove,
                        width,
                    ));
                }
            }
        }

        for link in &suggestion.documentation {
            output.push_str(&self.prefixed_rows(
                "DOCS   ",
                &self.theme.docs,
                &link.label,
                &self.theme.message,
                width,
            ));

            output.push_str(&self.prefixed_rows(
                "       ",
                &self.theme.docs,
                &link.url,
                &self.theme.docs,
                width,
            ));
        }

        for command in &suggestion.commands {
            output.push_str(&self.hard_prefixed_rows(
                "COMMAND ",
                &self.theme.command,
                &command.command,
                &self.theme.command,
                width,
            ));

            if let Some(explanation) = &command.explanation {
                output.push_str(&self.prefixed_rows(
                    "        ",
                    &self.theme.command,
                    explanation,
                    &self.theme.message,
                    width,
                ));
            }
        }

        output.push_str(&self.prefixed_rows(
            "APPLY  ",
            &self.theme.applicability,
            suggestion.applicability.as_str(),
            &self.theme.applicability,
            width,
        ));

        let fix_status = if suggestion.is_machine_applicable() && suggestion.has_edits() {
            "automatic fix available"
        } else {
            "manual review required"
        };

        output.push_str(&self.prefixed_rows(
            "FIX    ",
            &self.theme.fix,
            fix_status,
            &self.theme.fix,
            width,
        ));

        if !suggestion.commands.is_empty() {
            output.push_str(&self.prefixed_rows(
                "       ",
                &self.theme.command,
                "commands are suggestions only and are never executed automatically",
                &self.theme.message,
                width,
            ));
        }

        output
    }

    fn metadata_row(&self, label: &str, value: &str, width: usize) -> String {
        let prefix = format!("{label:<10}");

        self.prefixed_rows(
            &prefix,
            &self.theme.metadata_label,
            value,
            &self.theme.metadata_value,
            width,
        )
    }
}

impl TerminalRenderer {
    /// Renders a diagnostic using cached source text when available.
    ///
    /// Cached source has precedence over filesystem contents. If a source name
    /// is not present in the cache, rendering falls back to reading that name
    /// as a filesystem path, preserving the behavior of [`Renderer::render`].
    pub fn render_with_sources(&self, diagnostic: &Diagnostic, sources: &SourceCache) -> String {
        self.render_inner(diagnostic, Some(SourceStore::Cache(sources)))
    }

    /// Renders against an immutable point-in-time source snapshot.
    ///
    /// Snapshot contents take precedence over filesystem contents just like
    /// the live source cache, but cannot change after capture.
    pub fn render_with_snapshot(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
    ) -> String {
        self.render_inner(diagnostic, Some(SourceStore::Snapshot(sources)))
    }

    /// Renders a diagnostic against the exact snapshot captured with it.
    pub fn render_captured(&self, captured: &CapturedDiagnostic) -> String {
        self.render_with_snapshot(captured.diagnostic(), captured.sources())
    }

    fn render_inner(&self, diagnostic: &Diagnostic, sources: Option<SourceStore<'_>>) -> String {
        let width = self.effective_width();

        let icon = match diagnostic.severity {
            Severity::Trace => "·",
            Severity::Debug => "◆",
            Severity::Info => "ℹ",
            Severity::Warning => "⚠",
            Severity::Error => "✖",
            Severity::Fatal => "☠",
        };

        let raw_title = format!(
            "{icon} {}{}",
            diagnostic.severity,
            diagnostic
                .code
                .as_ref()
                .map(|code| format!(" [{code}]"))
                .unwrap_or_default()
        );

        let raw_title = truncate_visible(&raw_title, width.saturating_sub(5));

        let title = self.paint(self.theme.severity.style(diagnostic.severity), &raw_title);

        let title_width = visible_len(&title);

        let mut output = format!(
            "{}{title}{}\n",
            self.paint(&self.theme.border, "╭─ "),
            self.paint(
                &self.theme.border,
                &format!(" {}╮", "─".repeat(width.saturating_sub(title_width + 5)))
            )
        );

        output.push_str(&self.wrapped_rows(&diagnostic.message, &self.theme.message, width));

        if !diagnostic.labels.is_empty() {
            output.push_str(&self.row("", width));

            for source_line in self.source(diagnostic, width, sources) {
                output.push_str(&self.row(&source_line, width));
            }
        }

        if !diagnostic.attributes.is_empty() {
            output.push_str(&self.row("", width));

            for attribute in &diagnostic.attributes {
                output.push_str(&self.prefixed_rows(
                    "FIELD  ",
                    &self.theme.metadata_label,
                    &format!("{}={}", attribute.name, attribute.value,),
                    &self.theme.metadata_value,
                    width,
                ));
            }
        }

        if let Some(cause) = &diagnostic.cause {
            output.push_str(&self.row("", width));
            output.push_str(&self.row(&self.paint(&self.theme.cause, "CAUSE"), width));

            for (depth, cause) in cause.iter().enumerate() {
                let prefix = format!("{}└─ ", "   ".repeat(depth));

                output.push_str(&self.prefixed_rows(
                    &prefix,
                    &self.theme.cause,
                    &cause.message,
                    &self.theme.message,
                    width,
                ));
            }
        }

        if !diagnostic.notes.is_empty() {
            output.push_str(&self.row("", width));

            for note in &diagnostic.notes {
                output.push_str(&self.prefixed_rows(
                    "NOTE  ",
                    &self.theme.note,
                    note,
                    &self.theme.message,
                    width,
                ));
            }
        }

        if let Some(help) = &diagnostic.help {
            output.push_str(&self.row("", width));

            output.push_str(&self.prefixed_rows(
                "HELP  ",
                &self.theme.help,
                help,
                &self.theme.message,
                width,
            ));
        }

        for suggestion in &diagnostic.suggestions {
            output.push_str(&self.suggestion_rows(suggestion, width));
        }

        if self.show_metadata {
            output.push_str(&format!(
                "{}{}{}\n",
                self.paint(&self.theme.border, "├─ "),
                self.paint(&self.theme.metadata_label, "Diagnostic"),
                self.paint(
                    &self.theme.border,
                    &format!(" {}┤", "─".repeat(width.saturating_sub(15)))
                )
            ));

            output.push_str(&self.metadata_row(
                "Timestamp",
                &diagnostic.timestamp.to_rfc3339(),
                width,
            ));

            output.push_str(&self.metadata_row("App", &diagnostic.application, width));

            output.push_str(&self.metadata_row("PID", &diagnostic.pid.to_string(), width));

            output.push_str(&self.metadata_row("Host", &diagnostic.hostname, width));

            output.push_str(&self.metadata_row(
                "Session",
                &diagnostic.session_id.to_string(),
                width,
            ));

            output.push_str(&self.metadata_row("Report", &diagnostic.report_id.to_string(), width));
        }

        output.push_str(&format!(
            "{}\n",
            self.paint(
                &self.theme.border,
                &format!("╰{}╯", "─".repeat(width.saturating_sub(2)))
            )
        ));

        output
    }
}

impl Renderer for TerminalRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        self.render_inner(diagnostic, None)
    }
}
