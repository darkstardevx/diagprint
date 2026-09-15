use crate::DocumentationLink;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::{error::Error, fmt, time::Duration};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme as SyntectTheme, ThemeSet},
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

const DEFAULT_MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub enum TerminalDocError {
    Http(reqwest::Error),
    Html(html2text::Error),
    InvalidUrl(String),
    UnsupportedScheme(String),
    Selector(String),
    Highlight(String),

    DocumentTooLarge { limit: usize, actual: usize },
}

impl fmt::Display for TerminalDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(error) => write!(f, "{error}"),
            Self::Html(error) => write!(f, "{error}"),

            Self::InvalidUrl(error) => {
                write!(f, "invalid documentation URL: {error}")
            }

            Self::UnsupportedScheme(scheme) => {
                write!(f, "unsupported documentation URL scheme `{scheme}`")
            }

            Self::Selector(error) => write!(f, "{error}"),
            Self::Highlight(error) => write!(f, "{error}"),

            Self::DocumentTooLarge { limit, actual } => write!(
                f,
                "documentation response is too large: {actual} bytes \
                 exceeds the {limit}-byte limit"
            ),
        }
    }
}

impl Error for TerminalDocError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Html(error) => Some(error),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for TerminalDocError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

impl From<html2text::Error> for TerminalDocError {
    fn from(value: html2text::Error) -> Self {
        Self::Html(value)
    }
}

#[derive(Debug, Clone)]
pub struct TerminalDocViewer {
    width: usize,
    max_code_blocks: usize,
    max_document_bytes: usize,
    timeout: Duration,
    syntax_theme: String,
}

impl Default for TerminalDocViewer {
    fn default() -> Self {
        Self {
            width: 88,
            max_code_blocks: 8,
            max_document_bytes: DEFAULT_MAX_DOCUMENT_BYTES,
            timeout: Duration::from_secs(15),
            syntax_theme: "base16-ocean.dark".into(),
        }
    }
}

impl TerminalDocViewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn width(mut self, width: usize) -> Self {
        self.width = width.max(40);
        self
    }

    pub fn max_code_blocks(mut self, count: usize) -> Self {
        self.max_code_blocks = count;
        self
    }

    pub fn max_document_bytes(mut self, bytes: usize) -> Self {
        self.max_document_bytes = bytes.max(1024);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn syntax_theme(mut self, theme: impl Into<String>) -> Self {
        self.syntax_theme = theme.into();
        self
    }

    /// Fetches and renders a documentation link synchronously.
    ///
    /// This uses Reqwest's blocking client. Applications already running inside
    /// an async runtime should execute this method from an appropriate blocking
    /// worker instead of calling it directly on the async runtime thread.
    pub fn open(&self, link: &DocumentationLink) -> Result<String, TerminalDocError> {
        let url = reqwest::Url::parse(&link.url)
            .map_err(|error| TerminalDocError::InvalidUrl(error.to_string()))?;

        if !matches!(url.scheme(), "http" | "https") {
            return Err(TerminalDocError::UnsupportedScheme(url.scheme().to_owned()));
        }

        let client = Client::builder()
            .timeout(self.timeout)
            .user_agent(concat!(
                "diagprint/",
                env!("CARGO_PKG_VERSION"),
                " terminal-doc-viewer"
            ))
            .build()?;

        let response = client.get(url).send()?.error_for_status()?;

        if let Some(length) = response.content_length() {
            let length = usize::try_from(length).unwrap_or(usize::MAX);

            if length > self.max_document_bytes {
                return Err(TerminalDocError::DocumentTooLarge {
                    limit: self.max_document_bytes,
                    actual: length,
                });
            }
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let body = response.text()?;

        if body.len() > self.max_document_bytes {
            return Err(TerminalDocError::DocumentTooLarge {
                limit: self.max_document_bytes,
                actual: body.len(),
            });
        }

        let body = sanitize_terminal_input(&body);
        let label = sanitize_terminal_input(&link.label);
        let display_url = sanitize_terminal_input(&link.url);

        let mut output = String::new();

        output.push_str("\x1b[1;36mDOCS\x1b[0m  ");
        output.push_str(&label);
        output.push('\n');

        output.push_str("\x1b[2m");
        output.push_str(&display_url);
        output.push_str("\x1b[0m\n\n");

        let markdown = content_type.contains("markdown")
            || content_type.contains("text/plain")
            || link.url.ends_with(".md")
            || link.url.ends_with(".markdown");

        if markdown {
            output.push_str(&self.render_markdown(&body, link.language_hint.as_deref())?);
        } else {
            output.push_str(&self.render_html(&body, link.language_hint.as_deref())?);
        }

        Ok(output)
    }

    pub fn open_and_print(&self, link: &DocumentationLink) -> Result<(), TerminalDocError> {
        print!("{}", self.open(link)?);
        Ok(())
    }

    fn render_html(
        &self,
        html: &str,
        language_hint: Option<&str>,
    ) -> Result<String, TerminalDocError> {
        let document = Html::parse_document(html);

        let main_selector = Selector::parse("main")
            .map_err(|error| TerminalDocError::Selector(error.to_string()))?;

        let code_selector = Selector::parse("pre code")
            .map_err(|error| TerminalDocError::Selector(error.to_string()))?;

        let (content, code_blocks) = if let Some(main) = document.select(&main_selector).next() {
            let code_blocks = main
                .select(&code_selector)
                .take(self.max_code_blocks)
                .map(|element| {
                    let language = element
                        .value()
                        .attr("class")
                        .and_then(language_from_class)
                        .or(language_hint)
                        .unwrap_or("rust")
                        .to_owned();

                    let source = element.text().collect::<String>();

                    (language, source)
                })
                .collect::<Vec<_>>();

            (main.html(), code_blocks)
        } else {
            let code_blocks = document
                .select(&code_selector)
                .take(self.max_code_blocks)
                .map(|element| {
                    let language = element
                        .value()
                        .attr("class")
                        .and_then(language_from_class)
                        .or(language_hint)
                        .unwrap_or("rust")
                        .to_owned();

                    let source = element.text().collect::<String>();

                    (language, source)
                })
                .collect::<Vec<_>>();

            (html.to_owned(), code_blocks)
        };

        let mut output = html2text::from_read(content.as_bytes(), self.width)?;

        if !code_blocks.is_empty() {
            let syntaxes = SyntaxSet::load_defaults_newlines();
            let themes = ThemeSet::load_defaults();

            let theme = select_syntax_theme(&themes, &self.syntax_theme)?;

            output.push_str("\n\n\x1b[1;35mCODE EXAMPLES\x1b[0m\n");

            for (index, (language, code)) in code_blocks.iter().enumerate() {
                if code.trim().is_empty() {
                    continue;
                }

                output.push_str(&format!("\n\x1b[2m[{}] {}\x1b[0m\n", index + 1, language));

                output.push_str(&highlight_code(code, language, &syntaxes, theme)?);
            }
        }

        if !output.ends_with('\n') {
            output.push('\n');
        }

        Ok(output)
    }

    fn render_markdown(
        &self,
        markdown: &str,
        language_hint: Option<&str>,
    ) -> Result<String, TerminalDocError> {
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();

        let theme = select_syntax_theme(&themes, &self.syntax_theme)?;

        let mut output = String::new();
        let mut code = String::new();
        let mut in_code = false;
        let mut language = String::new();

        for line in markdown.lines() {
            if let Some(fence) = line.trim_start().strip_prefix("```") {
                if in_code {
                    output.push_str(&highlight_code(
                        &code,
                        if language.is_empty() {
                            language_hint.unwrap_or("txt")
                        } else {
                            &language
                        },
                        &syntaxes,
                        theme,
                    )?);

                    code.clear();
                    language.clear();
                    in_code = false;
                } else {
                    language = fence.trim().to_owned();
                    in_code = true;
                }

                continue;
            }

            if in_code {
                code.push_str(line);
                code.push('\n');
                continue;
            }

            if let Some(heading) = line.strip_prefix("### ") {
                output.push_str(&format!("\x1b[1;36m{heading}\x1b[0m\n"));
            } else if let Some(heading) = line.strip_prefix("## ") {
                output.push_str(&format!("\n\x1b[1;35m{heading}\x1b[0m\n"));
            } else if let Some(heading) = line.strip_prefix("# ") {
                output.push_str(&format!("\n\x1b[1;33m{heading}\x1b[0m\n"));
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        if in_code && !code.is_empty() {
            output.push_str(&highlight_code(
                &code,
                if language.is_empty() {
                    language_hint.unwrap_or("txt")
                } else {
                    &language
                },
                &syntaxes,
                theme,
            )?);
        }

        Ok(output)
    }
}

fn select_syntax_theme<'a>(
    themes: &'a ThemeSet,
    requested: &str,
) -> Result<&'a SyntectTheme, TerminalDocError> {
    themes
        .themes
        .get(requested)
        .or_else(|| themes.themes.get("base16-ocean.dark"))
        .or_else(|| themes.themes.values().next())
        .ok_or_else(|| {
            TerminalDocError::Highlight("syntect did not provide a terminal theme".into())
        })
}

fn language_from_class(class: &str) -> Option<&str> {
    class
        .split_whitespace()
        .find_map(|part| part.strip_prefix("language-"))
}

fn highlight_code(
    source: &str,
    language: &str,
    syntaxes: &SyntaxSet,
    theme: &SyntectTheme,
) -> Result<String, TerminalDocError> {
    let syntax = syntaxes
        .find_syntax_by_token(language)
        .or_else(|| syntaxes.find_syntax_by_extension(language))
        .or_else(|| {
            if matches!(language, "rs" | "rust") {
                syntaxes.find_syntax_by_extension("rs")
            } else {
                None
            }
        })
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    for line in LinesWithEndings::from(source) {
        let ranges = highlighter
            .highlight_line(line, syntaxes)
            .map_err(|error| TerminalDocError::Highlight(error.to_string()))?;

        output.push_str(&as_24_bit_terminal_escaped(&ranges, false));

        output.push_str("\x1b[0m");
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}

fn sanitize_terminal_input(input: &str) -> String {
    input
        .chars()
        .filter_map(|character| match character {
            '\n' | '\t' => Some(character),
            '\r' => None,
            '\x1b' => Some('␛'),
            character if character.is_control() => Some('�'),
            character => Some(character),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{TerminalDocViewer, sanitize_terminal_input};

    #[test]
    fn sanitizes_terminal_escape_sequences() {
        let input = "safe\x1b[31munsafe\x07text";

        let sanitized = sanitize_terminal_input(input);

        assert!(!sanitized.contains('\x1b'));
        assert!(!sanitized.contains('\x07'));
        assert!(sanitized.contains('␛'));
        assert!(sanitized.contains('�'));
    }

    #[test]
    fn renders_markdown_code_without_network() {
        let viewer = TerminalDocViewer::new();

        let Ok(rendered) = viewer.render_markdown(
            "# Example\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n",
            None,
        ) else {
            panic!("offline Markdown documentation rendering failed");
        };

        assert!(rendered.contains("Example"));
        assert!(rendered.contains("fn"));
        assert!(rendered.contains("println"));
        assert!(rendered.contains("\x1b["));
    }

    #[test]
    fn renders_html_main_content_without_network() {
        let viewer = TerminalDocViewer::new();

        let Ok(rendered) = viewer.render_html(
            r#"
                <html>
                    <body>
                        <nav>navigation noise</nav>
                        <main>
                            <h1>Diagnostic docs</h1>
                            <p>Useful explanation.</p>
                            <pre><code class="language-rust">fn demo() {}</code></pre>
                        </main>
                    </body>
                </html>
                "#,
            None,
        ) else {
            panic!("offline HTML documentation rendering failed");
        };

        assert!(rendered.contains("Diagnostic docs"));
        assert!(rendered.contains("Useful explanation"));
        assert!(rendered.contains("CODE EXAMPLES"));
        assert!(rendered.contains("fn"));
    }

    #[test]
    fn falls_back_when_syntax_theme_is_unknown() {
        let viewer = TerminalDocViewer::new().syntax_theme("__does_not_exist__");

        let Ok(rendered) = viewer.render_markdown("```rust\nlet value = 42;\n```\n", None) else {
            panic!("fallback syntax-theme rendering failed");
        };

        assert!(rendered.contains("value"));
    }
}
