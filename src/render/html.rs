use super::{
    Renderer,
    source_context::{
        DEFAULT_SOURCE_CONTEXT_LINES, SourceResolution, SourceStore, caret_line, resolve_source,
    },
};
use crate::{
    CapturedDiagnostic, Diagnostic, Label, LabelKind, Severity, SourceCache, SourceSnapshot,
    Suggestion,
};
use std::sync::OnceLock;
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style},
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

const SYNTECT_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "dp-" };

/// Visual theme used by standalone HTML diagnostic reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HtmlTheme {
    /// Follow the browser or operating-system light/dark preference.
    #[default]
    Auto,

    /// Force the light report theme.
    Light,

    /// Force the dark report theme.
    Dark,
}

impl HtmlTheme {
    const fn attribute(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// Source rendering options for [`HtmlRenderer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlSourceOptions {
    context_lines: usize,
    filesystem_fallback: bool,
}

impl Default for HtmlSourceOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlSourceOptions {
    /// Creates conservative source-rendering options.
    ///
    /// Two source lines are shown around the target and filesystem fallback is
    /// disabled.
    pub const fn new() -> Self {
        Self {
            context_lines: DEFAULT_SOURCE_CONTEXT_LINES,
            filesystem_fallback: false,
        }
    }

    /// Sets the number of source lines shown before and after the target line.
    pub const fn with_context_lines(mut self, context_lines: usize) -> Self {
        self.context_lines = context_lines;
        self
    }

    /// Enables or disables filesystem source fallback.
    ///
    /// This is disabled by default so rendering an HTML report does not
    /// unexpectedly embed local source files.
    pub const fn with_filesystem_fallback(mut self, enabled: bool) -> Self {
        self.filesystem_fallback = enabled;
        self
    }

    /// Number of context lines rendered around the target.
    pub const fn context_lines(self) -> usize {
        self.context_lines
    }

    /// Whether filesystem source fallback is enabled.
    pub const fn filesystem_fallback(self) -> bool {
        self.filesystem_fallback
    }
}

/// Self-contained HTML diagnostic renderer.
///
/// Generated documents contain all presentation CSS and syntax-highlight theme
/// rules inline. They require no external stylesheet, JavaScript, CDN, or
/// network access.
#[derive(Debug, Clone)]
pub struct HtmlRenderer {
    theme: HtmlTheme,
    show_metadata: bool,
    title: Option<String>,
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlRenderer {
    /// Creates an HTML renderer using automatic light/dark theme selection.
    pub const fn new() -> Self {
        Self {
            theme: HtmlTheme::Auto,
            show_metadata: true,
            title: None,
        }
    }

    /// Sets the visual report theme.
    pub const fn with_theme(mut self, theme: HtmlTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Controls whether collapsible diagnostic metadata is included.
    pub const fn with_metadata(mut self, enabled: bool) -> Self {
        self.show_metadata = enabled;
        self
    }

    /// Sets a custom page title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Renders one diagnostic using source text from a live cache.
    pub fn render_with_sources(&self, diagnostic: &Diagnostic, sources: &SourceCache) -> String {
        self.render_with_sources_and_options(diagnostic, sources, HtmlSourceOptions::default())
    }

    /// Renders one diagnostic using a live source cache and explicit options.
    pub fn render_with_sources_and_options(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceCache,
        options: HtmlSourceOptions,
    ) -> String {
        self.render_document(
            [diagnostic],
            Some(SourceStore::Cache(sources)),
            Some(options),
        )
    }

    /// Renders one diagnostic against an immutable source snapshot.
    pub fn render_with_snapshot(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
    ) -> String {
        self.render_with_snapshot_and_options(diagnostic, sources, HtmlSourceOptions::default())
    }

    /// Renders one diagnostic against an immutable snapshot with explicit
    /// options.
    pub fn render_with_snapshot_and_options(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
        options: HtmlSourceOptions,
    ) -> String {
        self.render_document(
            [diagnostic],
            Some(SourceStore::Snapshot(sources)),
            Some(options),
        )
    }

    /// Renders a captured diagnostic against the exact captured source state.
    pub fn render_captured(&self, captured: &CapturedDiagnostic) -> String {
        self.render_with_snapshot(captured.diagnostic(), captured.sources())
    }

    /// Explicitly opts into filesystem-backed source rendering.
    pub fn render_with_filesystem_sources(&self, diagnostic: &Diagnostic) -> String {
        self.render_document(
            [diagnostic],
            None,
            Some(HtmlSourceOptions::default().with_filesystem_fallback(true)),
        )
    }

    /// Renders a complete report using one live source cache.
    pub fn render_report_with_sources<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        sources: &SourceCache,
    ) -> String {
        self.render_document(
            diagnostics,
            Some(SourceStore::Cache(sources)),
            Some(HtmlSourceOptions::default()),
        )
    }

    /// Renders a complete report using one immutable source snapshot.
    pub fn render_report_with_snapshot<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        sources: &SourceSnapshot,
    ) -> String {
        self.render_document(
            diagnostics,
            Some(SourceStore::Snapshot(sources)),
            Some(HtmlSourceOptions::default()),
        )
    }

    /// Renders a complete report without embedding source text.
    pub fn render_report<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
    ) -> String {
        self.render_document(diagnostics, None, None)
    }

    fn render_document<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        sources: Option<SourceStore<'_>>,
        source_options: Option<HtmlSourceOptions>,
    ) -> String {
        let diagnostics = diagnostics.into_iter().collect::<Vec<_>>();

        let title = self
            .title
            .clone()
            .unwrap_or_else(|| default_title(diagnostics.len()));

        let mut output = String::new();

        output.push_str("<!doctype html>\n");
        output.push_str(&format!(
            "<html lang=\"en\" data-theme=\"{}\">\n",
            self.theme.attribute()
        ));
        output.push_str("<head>\n");
        output.push_str("<meta charset=\"utf-8\">\n");
        output
            .push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n");
        output.push_str("<meta name=\"color-scheme\" content=\"dark light\">\n");
        output.push_str(&format!("<title>{}</title>\n", escape_html(&title)));

        output.push_str("<style>\n");
        output.push_str(&syntax_theme_css(self.theme));
        output.push_str(BASE_CSS);
        output.push_str("\n</style>\n");
        output.push_str("</head>\n<body>\n");

        output.push_str("<main class=\"page-shell\">\n");
        output.push_str("<header class=\"page-header\">\n");
        output.push_str(
            "<div class=\"brand\"><span class=\"brand-mark\">dp</span> diagprint</div>\n",
        );
        output.push_str(&format!("<h1>{}</h1>\n", escape_html(&title)));

        output.push_str(&format!(
            "<p class=\"page-subtitle\">{} diagnostic{}</p>\n",
            diagnostics.len(),
            if diagnostics.len() == 1 { "" } else { "s" }
        ));

        output.push_str(&render_summary(&diagnostics));
        output.push_str("</header>\n");

        output.push_str("<div class=\"diagnostic-stack\">\n");

        for diagnostic in diagnostics {
            output.push_str(&self.render_card(diagnostic, sources, source_options));
        }

        output.push_str("</div>\n");

        output.push_str(
            "<footer class=\"page-footer\">Generated by <strong>diagprint</strong></footer>\n",
        );

        output.push_str("</main>\n");
        output.push_str("</body>\n</html>\n");

        output
    }

    fn render_card(
        &self,
        diagnostic: &Diagnostic,
        sources: Option<SourceStore<'_>>,
        source_options: Option<HtmlSourceOptions>,
    ) -> String {
        let severity = severity_token(diagnostic.severity);
        let mut output = String::new();

        output.push_str(&format!(
            "<article class=\"diagnostic severity-{severity}\">\n"
        ));

        output.push_str("<header class=\"diagnostic-header\">\n");
        output.push_str("<div class=\"diagnostic-heading-row\">\n");

        output.push_str(&format!(
            "<span class=\"severity-badge\">{} {}</span>\n",
            severity_icon(diagnostic.severity),
            escape_html(diagnostic.severity.as_str()),
        ));

        if let Some(code) = &diagnostic.code {
            output.push_str(&format!(
                "<code class=\"diagnostic-code\">{}</code>\n",
                escape_html(code)
            ));
        }

        output.push_str("</div>\n");

        output.push_str(&format!("<h2>{}</h2>\n", escape_html(&diagnostic.message)));

        output.push_str("</header>\n");

        if !diagnostic.attributes.is_empty() {
            output.push_str("<section class=\"section\">\n");
            output.push_str("<h3>Attributes</h3>\n");
            output.push_str("<div class=\"attribute-grid\">\n");

            for attribute in &diagnostic.attributes {
                output.push_str("<div class=\"attribute\">\n");
                output.push_str(&format!(
                    "<span>{}</span><code>{}</code>\n",
                    escape_html(&attribute.name),
                    escape_html(&attribute.value.to_string()),
                ));
                output.push_str("</div>\n");
            }

            output.push_str("</div>\n</section>\n");
        }

        if !diagnostic.labels.is_empty() {
            output.push_str("<section class=\"section\">\n");
            output.push_str("<h3>Locations</h3>\n");
            output.push_str("<div class=\"location-list\">\n");

            for label in &diagnostic.labels {
                output.push_str(&render_location(label));
            }

            output.push_str("</div>\n</section>\n");
        }

        if let Some(options) = source_options {
            output.push_str(&self.render_sources(diagnostic, sources, options));
        }

        if let Some(cause) = &diagnostic.cause {
            output.push_str("<section class=\"section\">\n");
            output.push_str("<h3>Cause chain</h3>\n");
            output.push_str("<ol class=\"cause-chain\">\n");

            for cause in cause.iter() {
                output.push_str(&format!("<li>{}</li>\n", escape_html(&cause.message)));
            }

            output.push_str("</ol>\n</section>\n");
        }

        if !diagnostic.notes.is_empty() {
            output.push_str("<section class=\"section\">\n");
            output.push_str("<h3>Notes</h3>\n");
            output.push_str("<ul class=\"notes\">\n");

            for note in &diagnostic.notes {
                output.push_str(&format!("<li>{}</li>\n", escape_html(note)));
            }

            output.push_str("</ul>\n</section>\n");
        }

        if let Some(help) = &diagnostic.help {
            output.push_str("<section class=\"callout help\">\n");
            output.push_str("<strong>Help</strong>\n");
            output.push_str(&format!("<p>{}</p>\n", escape_html(help)));
            output.push_str("</section>\n");
        }

        if !diagnostic.suggestions.is_empty() {
            output.push_str("<section class=\"section\">\n");
            output.push_str("<h3>Suggestions</h3>\n");
            output.push_str("<div class=\"suggestions\">\n");

            for suggestion in &diagnostic.suggestions {
                output.push_str(&render_suggestion(suggestion));
            }

            output.push_str("</div>\n</section>\n");
        }

        if self.show_metadata {
            output.push_str(&render_metadata(diagnostic));
        }

        output.push_str("</article>\n");

        output
    }

    fn render_sources(
        &self,
        diagnostic: &Diagnostic,
        sources: Option<SourceStore<'_>>,
        options: HtmlSourceOptions,
    ) -> String {
        if diagnostic.labels.is_empty() {
            return String::new();
        }

        let mut output = String::new();

        output.push_str("<section class=\"section source-section\">\n");
        output.push_str("<h3>Source</h3>\n");

        for label in &diagnostic.labels {
            output.push_str(&render_source_label(label, sources, options));
        }

        output.push_str("</section>\n");

        output
    }
}

impl Renderer for HtmlRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        self.render_document([diagnostic], None, None)
    }
}

fn render_source_label(
    label: &Label,
    sources: Option<SourceStore<'_>>,
    options: HtmlSourceOptions,
) -> String {
    let mut output = String::new();
    let kind = label_kind_token(label.kind);
    let location = label_location(label);

    output.push_str(&format!("<div class=\"source-panel source-{kind}\">\n"));

    output.push_str("<header class=\"source-header\">\n");

    output.push_str(&format!(
        "<span class=\"source-kind\">{}</span>\n",
        escape_html(kind)
    ));

    output.push_str(&format!("<code>{}</code>\n", escape_html(&location)));

    output.push_str("</header>\n");

    match resolve_source(
        label,
        sources,
        options.context_lines(),
        options.filesystem_fallback(),
    ) {
        SourceResolution::Available(context) => {
            let target_offset = context.target_line.saturating_sub(context.start_line);

            output.push_str(&format!(
                "<div class=\"source-meta\">\
                 <span>language: <code>{}</code></span>\
                 <span>lines <code>{}–{}</code></span>\
                 </div>\n",
                escape_html(context.language),
                context.start_line,
                context.end_line,
            ));

            let mut line_numbers = String::new();

            for line in context.start_line..=context.end_line {
                if !line_numbers.is_empty() {
                    line_numbers.push('\n');
                }

                if line == context.target_line {
                    line_numbers
                        .push_str(&format!("<span class=\"target-line-number\">{line}</span>"));
                } else {
                    line_numbers.push_str(&line.to_string());
                }
            }

            let highlighted = highlight_source(context.language, &context.text);

            output.push_str(&format!(
                "<div class=\"source-code\" style=\"--target-offset:{target_offset}\">\n\
                 <div class=\"source-target-band\" aria-hidden=\"true\"></div>\n\
                 <pre class=\"line-numbers\" aria-hidden=\"true\">{line_numbers}</pre>\n\
                 <pre class=\"code-block\" data-language=\"{}\"><code class=\"dp-code\">{highlighted}</code></pre>\n\
                 </div>\n",
                escape_html(context.language),
            ));

            let column = label.location.column.unwrap_or(1);
            let length = label.length.unwrap_or(1).max(1);

            let marker = match label.kind {
                LabelKind::Primary => '^',
                LabelKind::Secondary => '-',
            };

            let caret = caret_line(&context.target_text, column, length, marker);

            output.push_str("<div class=\"source-annotation\">\n");

            output.push_str(&format!(
                "<div class=\"span-location\">line <code>{}</code>, column <code>{column}</code>, length <code>{length}</code></div>\n",
                context.target_line,
            ));

            output.push_str(&format!(
                "<pre class=\"caret-line\"><code>{}</code></pre>\n",
                escape_html(&caret)
            ));

            if let Some(message) = &label.message {
                output.push_str(&format!("<p>{}</p>\n", escape_html(message)));
            }

            output.push_str("</div>\n");
        }

        SourceResolution::Stale { expected, actual } => {
            output.push_str(&warning_callout(&format!(
                "Stale source revision: diagnostic expects r{expected}, \
                 but the current source is r{actual}. Source text was not embedded."
            )));
        }

        SourceResolution::RevisionUnavailable { expected } => {
            output.push_str(&warning_callout(&format!(
                "Source revision unavailable: diagnostic expects r{expected}. \
                 A matching source cache or snapshot is required."
            )));
        }

        SourceResolution::LineUnavailable {
            requested,
            total_lines,
        } => {
            output.push_str(&warning_callout(&format!(
                "Source line unavailable: requested line {requested}, \
                 but the source contains {total_lines} lines."
            )));
        }

        SourceResolution::Unavailable => {
            output.push_str(
                "<div class=\"source-unavailable\">\
                 Source text unavailable. Structured location metadata is still preserved.\
                 </div>\n",
            );
        }
    }

    output.push_str("</div>\n");

    output
}

fn render_location(label: &Label) -> String {
    let kind = label_kind_token(label.kind);
    let mut output = String::new();

    output.push_str(&format!("<div class=\"location location-{kind}\">\n"));

    output.push_str(&format!(
        "<span class=\"location-kind\">{}</span>\n",
        escape_html(kind)
    ));

    output.push_str(&format!(
        "<code>{}</code>\n",
        escape_html(&label_location(label))
    ));

    if let Some(length) = label.length {
        output.push_str(&format!(
            "<span class=\"location-detail\">length {length}</span>\n"
        ));
    }

    if let Some(revision) = label.location.revision {
        output.push_str(&format!(
            "<span class=\"location-detail\">revision r{revision}</span>\n"
        ));
    }

    if let Some(message) = &label.message {
        output.push_str(&format!(
            "<span class=\"location-message\">{}</span>\n",
            escape_html(message)
        ));
    }

    output.push_str("</div>\n");

    output
}

fn render_suggestion(suggestion: &Suggestion) -> String {
    let mut output = String::new();

    output.push_str("<article class=\"suggestion\">\n");
    output.push_str("<header class=\"suggestion-header\">\n");

    output.push_str(&format!("<h4>{}</h4>\n", escape_html(&suggestion.title)));

    output.push_str(&format!(
        "<span class=\"applicability\">{}</span>\n",
        escape_html(suggestion.applicability.as_str())
    ));

    output.push_str("</header>\n");

    if let Some(explanation) = &suggestion.explanation {
        output.push_str(&format!("<p>{}</p>\n", escape_html(explanation)));
    }

    if !suggestion.documentation.is_empty() {
        output.push_str("<div class=\"suggestion-block\">\n");
        output.push_str("<strong>Documentation</strong>\n");
        output.push_str("<ul>\n");

        for link in &suggestion.documentation {
            output.push_str("<li>");

            if let Some(href) = safe_href(&link.url) {
                output.push_str(&format!(
                    "<a href=\"{}\" rel=\"noreferrer noopener\">{}</a>",
                    escape_html(href),
                    escape_html(&link.label),
                ));
            } else {
                output.push_str(&format!("<span>{}</span>", escape_html(&link.label)));
            }

            if let Some(language) = &link.language_hint {
                output.push_str(&format!(" <code>{}</code>", escape_html(language)));
            }

            output.push_str("</li>\n");
        }

        output.push_str("</ul>\n</div>\n");
    }

    if !suggestion.edits.is_empty() {
        output.push_str("<div class=\"suggestion-block\">\n");
        output.push_str("<strong>Proposed changes</strong>\n");

        for edit in &suggestion.edits {
            let preview = edit.preview_lines().join("\n");
            let highlighted = highlight_source("diff", &preview);

            output.push_str(&format!(
                "<pre class=\"diff-block\"><code class=\"dp-code\">{highlighted}</code></pre>\n"
            ));
        }

        output.push_str("</div>\n");
    }

    if !suggestion.commands.is_empty() {
        output.push_str("<div class=\"suggestion-block\">\n");
        output.push_str("<strong>Suggested commands</strong>\n");

        for command in &suggestion.commands {
            let highlighted = highlight_source("bash", &command.command);

            output.push_str(&format!(
                "<pre class=\"command-block\"><code class=\"dp-code\">{highlighted}</code></pre>\n"
            ));

            if let Some(explanation) = &command.explanation {
                output.push_str(&format!("<p>{}</p>\n", escape_html(explanation)));
            }
        }

        output.push_str(
            "<p class=\"safety-note\">Commands are advisory and are never executed automatically.</p>\n",
        );

        output.push_str("</div>\n");
    }

    output.push_str("</article>\n");

    output
}

fn render_metadata(diagnostic: &Diagnostic) -> String {
    let rows = [
        ("Application", diagnostic.application.clone()),
        ("Timestamp", diagnostic.timestamp.to_rfc3339()),
        ("PID", diagnostic.pid.to_string()),
        ("Host", diagnostic.hostname.clone()),
        ("Session", diagnostic.session_id.to_string()),
        ("Report", diagnostic.report_id.to_string()),
    ];

    let mut output = String::new();

    output.push_str("<details class=\"metadata\">\n");
    output.push_str("<summary>Diagnostic metadata</summary>\n");
    output.push_str("<dl>\n");

    for (name, value) in rows {
        output.push_str(&format!(
            "<dt>{}</dt><dd><code>{}</code></dd>\n",
            escape_html(name),
            escape_html(&value),
        ));
    }

    output.push_str("</dl>\n");
    output.push_str("</details>\n");

    output
}

fn render_summary(diagnostics: &[&Diagnostic]) -> String {
    let severities = [
        Severity::Trace,
        Severity::Debug,
        Severity::Info,
        Severity::Warning,
        Severity::Error,
        Severity::Fatal,
    ];

    let mut output = String::new();
    output.push_str("<div class=\"summary-chips\">\n");

    for severity in severities {
        let count = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == severity)
            .count();

        if count == 0 {
            continue;
        }

        output.push_str(&format!(
            "<span class=\"summary-chip severity-{}\">{} {} <strong>{count}</strong></span>\n",
            severity_token(severity),
            severity_icon(severity),
            escape_html(severity.as_str()),
        ));
    }

    output.push_str("</div>\n");

    output
}

fn warning_callout(message: &str) -> String {
    format!(
        "<div class=\"source-warning\">⚠ {}</div>\n",
        escape_html(message)
    )
}

fn default_title(count: usize) -> String {
    if count == 1 {
        "diagprint diagnostic report".to_owned()
    } else {
        format!("diagprint report — {count} diagnostics")
    }
}

fn label_location(label: &Label) -> String {
    format!(
        "{}:{}{}",
        label.location.file,
        label.location.line,
        label
            .location
            .column
            .map(|column| format!(":{column}"))
            .unwrap_or_default(),
    )
}

fn label_kind_token(kind: LabelKind) -> &'static str {
    match kind {
        LabelKind::Primary => "primary",
        LabelKind::Secondary => "secondary",
    }
}

fn severity_token(severity: Severity) -> &'static str {
    match severity {
        Severity::Trace => "trace",
        Severity::Debug => "debug",
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "fatal",
    }
}

fn severity_icon(severity: Severity) -> &'static str {
    match severity {
        Severity::Trace => "·",
        Severity::Debug => "◆",
        Severity::Info => "ℹ",
        Severity::Warning => "⚠",
        Severity::Error => "✖",
        Severity::Fatal => "☠",
    }
}

fn escape_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());

    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(ch),
        }
    }

    output
}

fn safe_href(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();

    if lower.starts_with("https://") || lower.starts_with("http://") {
        Some(trimmed)
    } else {
        None
    }
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn syntax_for_language<'a>(syntax_set: &'a SyntaxSet, language: &str) -> &'a SyntaxReference {
    let extension = match language {
        "rust" => "rs",
        "python" => "py",
        "javascript" => "js",
        "jsx" => "jsx",
        "typescript" => "ts",
        "tsx" => "tsx",
        "json" => "json",
        "toml" => "toml",
        "yaml" => "yaml",
        "markdown" => "md",
        "bash" | "shell" => "sh",
        "zsh" => "zsh",
        "fish" => "fish",
        "c" => "c",
        "cpp" => "cpp",
        "csharp" => "cs",
        "java" => "java",
        "kotlin" => "kt",
        "go" => "go",
        "ruby" => "rb",
        "php" => "php",
        "swift" => "swift",
        "lua" => "lua",
        "sql" => "sql",
        "html" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "xml" => "xml",
        "vue" => "vue",
        "svelte" => "svelte",
        "nix" => "nix",
        "protobuf" => "proto",
        "graphql" => "graphql",
        "ini" => "ini",
        "dotenv" => "env",
        "diff" => "diff",
        _ => "",
    };

    if extension.is_empty() {
        return syntax_set.find_syntax_plain_text();
    }

    syntax_set
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn highlight_source(language: &str, source: &str) -> String {
    let syntax_set = syntax_set();
    let syntax = syntax_for_language(syntax_set, language);

    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, syntax_set, SYNTECT_CLASS_STYLE);

    let mut normalized = source.to_owned();

    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }

    for line in LinesWithEndings::from(&normalized) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return escape_html(source);
        }
    }

    generator.finalize()
}

fn theme_css(name: &str) -> String {
    theme_set()
        .themes
        .get(name)
        .and_then(|theme| css_for_theme_with_class_style(theme, SYNTECT_CLASS_STYLE).ok())
        .unwrap_or_default()
}

fn syntax_theme_css(theme: HtmlTheme) -> String {
    const LIGHT_THEME: &str = "InspiredGitHub";
    const DARK_THEME: &str = "base16-ocean.dark";

    match theme {
        HtmlTheme::Light => theme_css(LIGHT_THEME),

        HtmlTheme::Dark => theme_css(DARK_THEME),

        HtmlTheme::Auto => {
            let light = theme_css(LIGHT_THEME);
            let dark = theme_css(DARK_THEME);

            format!(
                "@media (prefers-color-scheme: light) {{\n{light}\n}}\n\
                 @media (prefers-color-scheme: dark) {{\n{dark}\n}}\n"
            )
        }
    }
}

const BASE_CSS: &str = r#"
* {
  box-sizing: border-box;
}

:root {
  color-scheme: dark;
  --bg: #090d16;
  --surface: #101725;
  --surface-2: #151e2f;
  --surface-3: #1b2639;
  --border: #26344a;
  --border-soft: #1e2a3d;
  --text: #e7edf7;
  --muted: #95a4ba;
  --subtle: #6f8098;
  --code-bg: #0b111d;
  --shadow: 0 18px 50px rgba(0, 0, 0, .28);

  --trace: #8b98aa;
  --debug: #b18cff;
  --info: #55c7ff;
  --warning: #f5c451;
  --error: #ff667c;
  --fatal: #ff3ca6;

  --primary: #55c7ff;
  --secondary: #9aa9bd;
  --good: #5ee6a8;
}

:root[data-theme="light"] {
  color-scheme: light;
  --bg: #f3f6fb;
  --surface: #ffffff;
  --surface-2: #f6f8fc;
  --surface-3: #edf2f8;
  --border: #d7dfeb;
  --border-soft: #e3e9f1;
  --text: #172033;
  --muted: #56657a;
  --subtle: #758399;
  --code-bg: #f5f7fa;
  --shadow: 0 18px 50px rgba(35, 50, 75, .10);

  --trace: #657287;
  --debug: #754fd4;
  --info: #087db5;
  --warning: #a96a00;
  --error: #cc304a;
  --fatal: #c00075;

  --primary: #087db5;
  --secondary: #64748b;
  --good: #168557;
}

@media (prefers-color-scheme: light) {
  :root[data-theme="auto"] {
    color-scheme: light;
    --bg: #f3f6fb;
    --surface: #ffffff;
    --surface-2: #f6f8fc;
    --surface-3: #edf2f8;
    --border: #d7dfeb;
    --border-soft: #e3e9f1;
    --text: #172033;
    --muted: #56657a;
    --subtle: #758399;
    --code-bg: #f5f7fa;
    --shadow: 0 18px 50px rgba(35, 50, 75, .10);

    --trace: #657287;
    --debug: #754fd4;
    --info: #087db5;
    --warning: #a96a00;
    --error: #cc304a;
    --fatal: #c00075;

    --primary: #087db5;
    --secondary: #64748b;
    --good: #168557;
  }
}

:root[data-theme="dark"] {
  color-scheme: dark;
}

body {
  margin: 0;
  background:
    radial-gradient(circle at top right, rgba(85, 199, 255, .08), transparent 28rem),
    radial-gradient(circle at top left, rgba(255, 60, 166, .055), transparent 24rem),
    var(--bg);
  color: var(--text);
  font-family:
    Inter,
    ui-sans-serif,
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    sans-serif;
  line-height: 1.55;
}

code,
pre {
  font-family:
    "JetBrains Mono",
    "Cascadia Code",
    "SFMono-Regular",
    Consolas,
    "Liberation Mono",
    monospace;
}

code {
  font-size: .92em;
}

.page-shell {
  width: min(1120px, calc(100% - 2rem));
  margin: 0 auto;
  padding: 3rem 0 4rem;
}

.page-header {
  margin-bottom: 2rem;
}

.brand {
  display: flex;
  align-items: center;
  gap: .65rem;
  color: var(--muted);
  font-weight: 700;
  letter-spacing: .08em;
  text-transform: uppercase;
  font-size: .8rem;
}

.brand-mark {
  display: inline-grid;
  place-items: center;
  width: 2rem;
  height: 2rem;
  border: 1px solid var(--primary);
  border-radius: .55rem;
  color: var(--primary);
  background: color-mix(in srgb, var(--primary) 8%, transparent);
  text-transform: lowercase;
  letter-spacing: 0;
}

.page-header h1 {
  margin: 1rem 0 .2rem;
  font-size: clamp(2rem, 5vw, 3.5rem);
  line-height: 1.05;
  letter-spacing: -.04em;
}

.page-subtitle {
  margin: .5rem 0 1rem;
  color: var(--muted);
}

.summary-chips {
  display: flex;
  flex-wrap: wrap;
  gap: .55rem;
}

.summary-chip {
  --accent: var(--muted);
  display: inline-flex;
  align-items: center;
  gap: .4rem;
  border: 1px solid color-mix(in srgb, var(--accent) 38%, var(--border));
  background: color-mix(in srgb, var(--accent) 8%, var(--surface));
  color: var(--accent);
  border-radius: 999px;
  padding: .36rem .72rem;
  font-size: .76rem;
  font-weight: 700;
  letter-spacing: .045em;
}

.summary-chip strong {
  color: var(--text);
}

.diagnostic-stack {
  display: grid;
  gap: 1.35rem;
}

.diagnostic {
  --accent: var(--muted);
  position: relative;
  overflow: hidden;
  background: color-mix(in srgb, var(--surface) 97%, transparent);
  border: 1px solid var(--border);
  border-radius: 1rem;
  box-shadow: var(--shadow);
}

.diagnostic::before {
  content: "";
  position: absolute;
  inset: 0 auto 0 0;
  width: .28rem;
  background: var(--accent);
}

.severity-trace {
  --accent: var(--trace);
}

.severity-debug {
  --accent: var(--debug);
}

.severity-info {
  --accent: var(--info);
}

.severity-warning {
  --accent: var(--warning);
}

.severity-error {
  --accent: var(--error);
}

.severity-fatal {
  --accent: var(--fatal);
}

.diagnostic-header {
  padding: 1.45rem 1.55rem 1.2rem;
  border-bottom: 1px solid var(--border-soft);
  background:
    linear-gradient(
      120deg,
      color-mix(in srgb, var(--accent) 8%, transparent),
      transparent 45%
    );
}

.diagnostic-heading-row {
  display: flex;
  flex-wrap: wrap;
  gap: .7rem;
  align-items: center;
}

.severity-badge {
  color: var(--accent);
  border: 1px solid color-mix(in srgb, var(--accent) 46%, var(--border));
  background: color-mix(in srgb, var(--accent) 10%, transparent);
  border-radius: 999px;
  padding: .32rem .68rem;
  font-size: .75rem;
  font-weight: 800;
  letter-spacing: .07em;
}

.diagnostic-code {
  color: var(--muted);
  background: var(--surface-2);
  border: 1px solid var(--border-soft);
  border-radius: .45rem;
  padding: .22rem .48rem;
}

.diagnostic-header h2 {
  margin: .95rem 0 0;
  max-width: 70ch;
  font-size: 1.25rem;
  line-height: 1.45;
}

.section {
  padding: 1.2rem 1.55rem;
  border-bottom: 1px solid var(--border-soft);
}

.section h3 {
  margin: 0 0 .85rem;
  color: var(--muted);
  font-size: .78rem;
  text-transform: uppercase;
  letter-spacing: .085em;
}

.attribute-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
  gap: .65rem;
}

.attribute {
  display: flex;
  justify-content: space-between;
  gap: .75rem;
  align-items: center;
  border: 1px solid var(--border-soft);
  background: var(--surface-2);
  border-radius: .65rem;
  padding: .58rem .7rem;
}

.attribute span {
  color: var(--muted);
}

.attribute code {
  overflow-wrap: anywhere;
}

.location-list {
  display: grid;
  gap: .55rem;
}

.location {
  --location-accent: var(--secondary);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: .55rem;
  padding: .6rem .75rem;
  border-radius: .65rem;
  background: var(--surface-2);
  border: 1px solid var(--border-soft);
}

.location-primary {
  --location-accent: var(--primary);
}

.location-kind,
.source-kind {
  color: var(--location-accent);
  text-transform: uppercase;
  font-size: .7rem;
  letter-spacing: .075em;
  font-weight: 800;
}

.location-detail {
  color: var(--subtle);
  font-size: .8rem;
}

.location-message {
  color: var(--muted);
}

.source-section {
  display: grid;
  gap: 1rem;
}

.source-panel {
  --location-accent: var(--secondary);
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: .8rem;
  background: var(--surface-2);
}

.source-primary {
  --location-accent: var(--primary);
}

.source-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: .65rem;
  padding: .65rem .8rem;
  border-bottom: 1px solid var(--border-soft);
}

.source-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  padding: .5rem .8rem;
  color: var(--subtle);
  font-size: .78rem;
  border-bottom: 1px solid var(--border-soft);
}

.source-code {
  --source-line-height: 1.55em;
  position: relative;
  display: grid;
  grid-template-columns: auto 1fr;
  overflow-x: auto;
  background: var(--code-bg);
}

.source-target-band {
  position: absolute;
  z-index: 0;
  top: calc(.85rem + var(--target-offset) * var(--source-line-height));
  left: 0;
  right: 0;
  height: var(--source-line-height);
  pointer-events: none;
  background: color-mix(in srgb, var(--location-accent) 10%, transparent);
  border-left: .2rem solid var(--location-accent);
}

.source-code pre {
  position: relative;
  z-index: 1;
  margin: 0;
  line-height: var(--source-line-height);
  background: transparent;
}

.line-numbers {
  min-width: 3.5rem;
  padding: .85rem .72rem;
  color: var(--subtle);
  text-align: right;
  user-select: none;
  border-right: 1px solid var(--border-soft);
}

.target-line-number {
  color: var(--location-accent);
  font-weight: 800;
}

.code-block {
  min-width: max-content;
  padding: .85rem 1rem;
}

.dp-code {
  background: transparent !important;
}

.source-annotation {
  padding: .72rem .8rem;
  border-top: 1px solid var(--border-soft);
}

.span-location {
  color: var(--subtle);
  font-size: .8rem;
}

.caret-line {
  margin: .45rem 0 .25rem;
  overflow-x: auto;
  color: var(--location-accent);
  background: transparent;
  line-height: 1.3;
}

.source-annotation p {
  margin: .35rem 0 0;
  color: var(--muted);
}

.source-warning,
.source-unavailable {
  padding: .8rem;
  color: var(--warning);
}

.source-unavailable {
  color: var(--muted);
}

.cause-chain,
.notes {
  margin: 0;
  padding-left: 1.3rem;
}

.cause-chain li + li,
.notes li + li {
  margin-top: .45rem;
}

.callout {
  margin: 0;
  padding: 1rem 1.55rem;
  border-bottom: 1px solid var(--border-soft);
}

.callout.help {
  background: color-mix(in srgb, var(--good) 6%, transparent);
  border-left: .22rem solid var(--good);
}

.callout p {
  margin: .35rem 0 0;
}

.suggestions {
  display: grid;
  gap: .85rem;
}

.suggestion {
  padding: .9rem;
  border: 1px solid var(--border);
  border-radius: .75rem;
  background: var(--surface-2);
}

.suggestion-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: .8rem;
  flex-wrap: wrap;
}

.suggestion-header h4 {
  margin: 0;
}

.applicability {
  color: var(--good);
  border: 1px solid color-mix(in srgb, var(--good) 38%, var(--border));
  border-radius: 999px;
  padding: .22rem .52rem;
  font-size: .7rem;
  font-weight: 700;
}

.suggestion-block {
  margin-top: .85rem;
}

.suggestion-block > strong {
  display: block;
  margin-bottom: .45rem;
  color: var(--muted);
}

.suggestion a {
  color: var(--primary);
}

.diff-block,
.command-block {
  overflow-x: auto;
  padding: .75rem;
  border: 1px solid var(--border-soft);
  border-radius: .6rem;
  background: var(--code-bg);
}

.safety-note {
  color: var(--subtle);
  font-size: .8rem;
}

.metadata {
  padding: .9rem 1.55rem 1rem;
}

.metadata summary {
  cursor: pointer;
  color: var(--muted);
  font-size: .8rem;
  font-weight: 650;
}

.metadata dl {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: .4rem 1rem;
  margin: .9rem 0 0;
}

.metadata dt {
  color: var(--subtle);
}

.metadata dd {
  margin: 0;
  min-width: 0;
}

.metadata dd code {
  overflow-wrap: anywhere;
}

.page-footer {
  margin-top: 2rem;
  color: var(--subtle);
  text-align: center;
  font-size: .78rem;
}

@media (max-width: 680px) {
  .page-shell {
    width: min(100% - 1rem, 1120px);
    padding-top: 1.25rem;
  }

  .diagnostic-header,
  .section,
  .callout,
  .metadata {
    padding-left: 1rem;
    padding-right: 1rem;
  }

  .metadata dl {
    grid-template-columns: 1fr;
    gap: .15rem;
  }

  .metadata dd + dt {
    margin-top: .5rem;
  }
}

@media print {
  :root {
    color-scheme: light;
  }

  body {
    background: #fff;
  }

  .page-shell {
    width: 100%;
    padding: 0;
  }

  .diagnostic {
    box-shadow: none;
    break-inside: avoid;
  }
}
"#;
