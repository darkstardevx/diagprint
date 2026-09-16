use super::{
    Renderer,
    source_context::{
        DEFAULT_SOURCE_CONTEXT_LINES, SourceResolution, SourceStore, caret_line, resolve_source,
    },
};
use crate::{
    CapturedDiagnostic, Diagnostic, Label, LabelKind, SourceCache, SourceSnapshot, Suggestion,
};

/// Markdown rendering options for embedded source context.
///
/// Source embedding is explicit. Ordinary [`Renderer::render`] output does not
/// automatically read or embed source contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownSourceOptions {
    context_lines: usize,
    filesystem_fallback: bool,
}

impl Default for MarkdownSourceOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownSourceOptions {
    /// Creates conservative source-rendering options.
    ///
    /// Two context lines are included on each side of the target line and
    /// filesystem fallback is disabled.
    pub const fn new() -> Self {
        Self {
            context_lines: DEFAULT_SOURCE_CONTEXT_LINES,
            filesystem_fallback: false,
        }
    }

    /// Sets the number of source lines shown before and after the target.
    pub const fn with_context_lines(mut self, context_lines: usize) -> Self {
        self.context_lines = context_lines;
        self
    }

    /// Controls whether a missing cached source may be read from the
    /// filesystem.
    ///
    /// This is disabled by default because Markdown reports are commonly
    /// persisted or shared.
    pub const fn with_filesystem_fallback(mut self, enabled: bool) -> Self {
        self.filesystem_fallback = enabled;
        self
    }

    /// Number of source lines shown before and after the target.
    pub const fn context_lines(self) -> usize {
        self.context_lines
    }

    /// Whether filesystem source fallback is enabled.
    pub const fn filesystem_fallback(self) -> bool {
        self.filesystem_fallback
    }
}

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

fn max_run(text: &str, target: char) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for ch in text.chars() {
        if ch == target {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}

fn fenced_block(language: &str, text: &str) -> String {
    let fence_length = max_run(text, '~').saturating_add(1).max(3);
    let fence = "~".repeat(fence_length);

    format!("{fence}{language}\n{text}\n{fence}\n")
}

fn render_suggestion(output: &mut String, suggestion: &Suggestion) {
    output.push_str(&format!("### {}\n\n", suggestion.title));
    output.push_str(&format!(
        "**Applicability:** `{}`\n\n",
        suggestion.applicability.as_str()
    ));

    if let Some(explanation) = &suggestion.explanation {
        output.push_str(explanation);
        output.push_str("\n\n");
    }

    if !suggestion.documentation.is_empty() {
        output.push_str("#### Documentation\n\n");

        for link in &suggestion.documentation {
            output.push_str(&format!("- [{}]({})", link.label, link.url));

            if let Some(language) = &link.language_hint {
                output.push_str(&format!(" — `{language}`"));
            }

            output.push('\n');
        }

        output.push('\n');
    }

    if !suggestion.edits.is_empty() {
        output.push_str("#### Proposed changes\n\n");

        for edit in &suggestion.edits {
            let preview = edit.preview_lines().join("\n");

            output.push_str(&fenced_block("diff", &preview));
            output.push('\n');
        }
    }

    if !suggestion.commands.is_empty() {
        output.push_str("#### Suggested commands\n\n");

        for command in &suggestion.commands {
            output.push_str(&fenced_block("shell", &command.command));

            if let Some(explanation) = &command.explanation {
                output.push('\n');
                output.push_str(explanation);
                output.push_str("\n\n");
            }
        }

        output.push_str(
            "> Suggested commands are informational and are never executed automatically.\n\n",
        );
    }
}

impl MarkdownRenderer {
    /// Renders using source text from a live cache.
    ///
    /// Filesystem fallback remains disabled unless explicitly enabled through
    /// [`MarkdownSourceOptions`].
    pub fn render_with_sources(&self, diagnostic: &Diagnostic, sources: &SourceCache) -> String {
        self.render_with_sources_and_options(diagnostic, sources, MarkdownSourceOptions::default())
    }

    /// Renders using source text from a live cache and explicit options.
    pub fn render_with_sources_and_options(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceCache,
        options: MarkdownSourceOptions,
    ) -> String {
        self.render_inner(diagnostic, Some(SourceStore::Cache(sources)), Some(options))
    }

    /// Renders against an immutable source snapshot.
    pub fn render_with_snapshot(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
    ) -> String {
        self.render_with_snapshot_and_options(diagnostic, sources, MarkdownSourceOptions::default())
    }

    /// Renders against an immutable source snapshot and explicit options.
    pub fn render_with_snapshot_and_options(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
        options: MarkdownSourceOptions,
    ) -> String {
        self.render_inner(
            diagnostic,
            Some(SourceStore::Snapshot(sources)),
            Some(options),
        )
    }

    /// Renders a captured diagnostic against its exact captured source state.
    pub fn render_captured(&self, captured: &CapturedDiagnostic) -> String {
        self.render_with_snapshot(captured.diagnostic(), captured.sources())
    }

    /// Explicitly renders source context from filesystem paths stored in
    /// diagnostic labels.
    ///
    /// This method opts into filesystem reads. Revisioned labels still require
    /// a cache or snapshot so the requested revision can be verified.
    pub fn render_with_filesystem_sources(&self, diagnostic: &Diagnostic) -> String {
        self.render_inner(
            diagnostic,
            None,
            Some(MarkdownSourceOptions::default().with_filesystem_fallback(true)),
        )
    }

    /// Renders a complete Markdown report using one live source cache.
    pub fn render_report_with_sources<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        sources: &SourceCache,
    ) -> String {
        diagnostics
            .into_iter()
            .map(|diagnostic| self.render_with_sources(diagnostic, sources))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }

    /// Renders a complete Markdown report using one immutable source snapshot.
    pub fn render_report_with_snapshot<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        sources: &SourceSnapshot,
    ) -> String {
        diagnostics
            .into_iter()
            .map(|diagnostic| self.render_with_snapshot(diagnostic, sources))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }

    fn render_inner(
        &self,
        diagnostic: &Diagnostic,
        sources: Option<SourceStore<'_>>,
        source_options: Option<MarkdownSourceOptions>,
    ) -> String {
        let mut output = format!(
            "# {}{}\n\n\
             **Application:** `{}`  \n\
             **Timestamp:** `{}`  \n\
             **PID:** `{}`  \n\
             **Host:** `{}`  \n\
             **Session:** `{}`  \n\
             **Report:** `{}`\n\n\
             ## Message\n\n\
             {}\n",
            diagnostic.severity,
            diagnostic
                .code
                .as_ref()
                .map(|code| format!(" — `{code}`"))
                .unwrap_or_default(),
            diagnostic.application,
            diagnostic.timestamp.to_rfc3339(),
            diagnostic.pid,
            diagnostic.hostname,
            diagnostic.session_id,
            diagnostic.report_id,
            diagnostic.message,
        );

        if !diagnostic.attributes.is_empty() {
            output.push_str("\n## Attributes\n\n");

            for attribute in &diagnostic.attributes {
                output.push_str(&format!(
                    "- **{}:** `{}`\n",
                    attribute.name, attribute.value
                ));
            }
        }

        if !diagnostic.labels.is_empty() {
            output.push_str("\n## Labels\n\n");

            for label in &diagnostic.labels {
                output.push_str(&format!(
                    "- **{}:** `{}`",
                    label_kind_name(label.kind),
                    label_location(label)
                ));

                if let Some(length) = label.length {
                    output.push_str(&format!(" (length: `{length}`)"));
                }

                if let Some(revision) = label.location.revision {
                    output.push_str(&format!(" (revision: `r{revision}`)"));
                }

                if let Some(message) = &label.message {
                    output.push_str(" — ");
                    output.push_str(message);
                }

                output.push('\n');
            }
        }

        if let Some(options) = source_options {
            self.render_sources(&mut output, diagnostic, sources, options);
        }

        if let Some(cause) = &diagnostic.cause {
            output.push_str("\n## Causes\n\n");

            for (depth, cause) in cause.iter().enumerate() {
                output.push_str(&format!("{}- {}\n", "  ".repeat(depth), cause.message));
            }
        }

        if !diagnostic.notes.is_empty() {
            output.push_str("\n## Notes\n\n");

            for note in &diagnostic.notes {
                output.push_str(&format!("- {note}\n"));
            }
        }

        if let Some(help) = &diagnostic.help {
            output.push_str(&format!("\n## Help\n\n{help}\n"));
        }

        if !diagnostic.suggestions.is_empty() {
            output.push_str("\n## Suggestions\n\n");

            for suggestion in &diagnostic.suggestions {
                render_suggestion(&mut output, suggestion);
            }
        }

        output
    }

    fn render_sources(
        &self,
        output: &mut String,
        diagnostic: &Diagnostic,
        sources: Option<SourceStore<'_>>,
        options: MarkdownSourceOptions,
    ) {
        if diagnostic.labels.is_empty() {
            return;
        }

        output.push_str("\n## Source\n\n");

        for label in &diagnostic.labels {
            let kind = label_kind_name(label.kind);
            let location = label_location(label);

            output.push_str(&format!("### {kind} — `{location}`\n\n"));

            match resolve_source(
                label,
                sources,
                options.context_lines(),
                options.filesystem_fallback(),
            ) {
                SourceResolution::Available(context) => {
                    output.push_str(&format!(
                        "**Language:** `{}` · **Context:** lines `{}–{}`\n\n",
                        context.language, context.start_line, context.end_line,
                    ));

                    output.push_str(&fenced_block(context.language, &context.text));
                    output.push('\n');

                    let column = label.location.column.unwrap_or(1);
                    let length = label.length.unwrap_or(1).max(1);

                    output.push_str(&format!(
                        "> **{kind} span:** line `{}`, column `{column}`, length `{length}`",
                        context.target_line,
                    ));

                    if let Some(message) = &label.message {
                        output.push_str(" — ");
                        output.push_str(message);
                    }

                    output.push_str("\n\n");

                    let marker = match label.kind {
                        LabelKind::Primary => '^',
                        LabelKind::Secondary => '-',
                    };

                    let mut annotation = caret_line(&context.target_text, column, length, marker);

                    if let Some(message) = &label.message {
                        annotation.push(' ');
                        annotation.push_str(message);
                    }

                    output.push_str(&fenced_block("text", &annotation));
                    output.push('\n');
                }

                SourceResolution::Stale { expected, actual } => {
                    output.push_str(&format!(
                        "> ⚠️ **Stale source revision:** diagnostic expects `r{expected}`, \
                         but the current source is `r{actual}`. Source text was not embedded.\n\n"
                    ));
                }

                SourceResolution::RevisionUnavailable { expected } => {
                    output.push_str(&format!(
                        "> ⚠️ **Source revision unavailable:** diagnostic expects `r{expected}`. \
                         A matching source cache or snapshot is required.\n\n"
                    ));
                }

                SourceResolution::LineUnavailable {
                    requested,
                    total_lines,
                } => {
                    output.push_str(&format!(
                        "> ⚠️ **Source line unavailable:** requested line `{requested}`, \
                         but the source contains `{total_lines}` lines.\n\n"
                    ));
                }

                SourceResolution::Unavailable => {
                    output.push_str(
                        "> Source text unavailable. Only structured location metadata is shown.\n\n",
                    );
                }
            }
        }
    }
}

impl Renderer for MarkdownRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        self.render_inner(diagnostic, None, None)
    }
}
