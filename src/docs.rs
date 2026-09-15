use crate::DocumentationLink;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::{error::Error, fmt, time::Duration};
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};

#[derive(Debug)]
pub enum TerminalDocError {
    Http(reqwest::Error),
    Html(html2text::Error),
    Selector(String),
    Highlight(String),
}

impl fmt::Display for TerminalDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(error) => write!(f, "{error}"),
            Self::Html(error) => write!(f, "{error}"),
            Self::Selector(error) => write!(f, "{error}"),
            Self::Highlight(error) => write!(f, "{error}"),
        }
    }
}

impl Error for TerminalDocError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Html(error) => Some(error),
            Self::Selector(_) | Self::Highlight(_) => None,
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
    timeout: Duration,
}

impl Default for TerminalDocViewer {
    fn default() -> Self {
        Self {
            width: 88,
            max_code_blocks: 8,
            timeout: Duration::from_secs(15),
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

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn open(&self, link: &DocumentationLink) -> Result<String, TerminalDocError> {
        let client = Client::builder()
            .timeout(self.timeout)
            .user_agent(concat!(
                "diagprint/",
                env!("CARGO_PKG_VERSION"),
                " terminal-doc-viewer"
            ))
            .build()?;

        let response = client.get(&link.url).send()?.error_for_status()?;

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let body = response.text()?;

        let mut output = String::new();

        output.push_str("\x1b[1;36mDOCS\x1b[0m  ");
        output.push_str(&link.label);
        output.push('\n');

        output.push_str("\x1b[2m");
        output.push_str(&link.url);
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

        let content = document
            .select(&main_selector)
            .next()
            .map(|element| element.html())
            .unwrap_or_else(|| html.to_owned());

        let mut output = html2text::from_read(content.as_bytes(), self.width)?;

        let code_selector = Selector::parse("pre code")
            .map_err(|error| TerminalDocError::Selector(error.to_string()))?;

        let blocks: Vec<_> = document
            .select(&code_selector)
            .take(self.max_code_blocks)
            .collect();

        if !blocks.is_empty() {
            output.push_str("\n\n\x1b[1;35mCODE EXAMPLES\x1b[0m\n");

            for (index, element) in blocks.into_iter().enumerate() {
                let code = element.text().collect::<String>();

                if code.trim().is_empty() {
                    continue;
                }

                let language = element
                    .value()
                    .attr("class")
                    .and_then(language_from_class)
                    .or(language_hint)
                    .unwrap_or("rust");

                output.push_str(&format!("\n\x1b[2m[{}] {language}\x1b[0m\n", index + 1));

                output.push_str(&highlight_code(&code, language)?);
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
            )?);
        }

        Ok(output)
    }
}

fn language_from_class(class: &str) -> Option<&str> {
    class
        .split_whitespace()
        .find_map(|part| part.strip_prefix("language-"))
}

fn highlight_code(source: &str, language: &str) -> Result<String, TerminalDocError> {
    let syntaxes = SyntaxSet::load_defaults_newlines();
    let themes = ThemeSet::load_defaults();

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

    let theme = themes
        .themes
        .get("base16-ocean.dark")
        .or_else(|| themes.themes.values().next())
        .ok_or_else(|| {
            TerminalDocError::Highlight("syntect did not provide a terminal theme".into())
        })?;

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    for line in LinesWithEndings::from(source) {
        let ranges = highlighter
            .highlight_line(line, &syntaxes)
            .map_err(|error| TerminalDocError::Highlight(error.to_string()))?;

        output.push_str(&as_24_bit_terminal_escaped(&ranges, false));

        output.push_str("\x1b[0m");
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}
