mod github_actions;
mod json;
mod markdown;
mod plain;
mod sarif;
mod terminal;
mod theme;

pub use github_actions::GithubActionsRenderer;
pub use json::JsonRenderer;
pub use markdown::MarkdownRenderer;
pub use plain::PlainRenderer;
pub use sarif::SarifRenderer;
pub use terminal::TerminalRenderer;
pub use theme::{SeverityTheme, Style, Theme};

use crate::{Diagnostic, ExportDiagnostic};

pub trait Renderer {
    fn render(&self, diagnostic: &Diagnostic) -> String;
}

/// Renders multiple diagnostics as one logical output document.
///
/// Implementations must produce a valid representation for the corresponding
/// format rather than merely concatenating representations when that would
/// make the result invalid.
pub trait ReportRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String;
}

fn render_joined<'a>(
    renderer: &impl Renderer,
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
    separator: &str,
) -> String {
    diagnostics
        .into_iter()
        .map(|diagnostic| renderer.render(diagnostic))
        .collect::<Vec<_>>()
        .join(separator)
}

impl ReportRenderer for PlainRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        render_joined(self, diagnostics, "\n")
    }
}

impl ReportRenderer for GithubActionsRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        render_joined(self, diagnostics, "\n")
    }
}

impl ReportRenderer for MarkdownRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        render_joined(self, diagnostics, "\n\n---\n\n")
    }
}

impl ReportRenderer for JsonRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        let diagnostics = diagnostics
            .into_iter()
            .map(ExportDiagnostic::from)
            .collect::<Vec<_>>();

        serde_json::to_string_pretty(&diagnostics).expect("diagnostic report serialization failed")
    }
}

impl ReportRenderer for SarifRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        self.render_many(diagnostics)
    }
}
