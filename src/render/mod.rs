mod audit_transcript;
mod compiler_text;
mod github_actions;
mod github_actions_delta;

#[cfg(feature = "html")]
mod html;

mod json;
mod markdown;
mod plain;
mod rendered_artifact;
mod sarif;
mod source_context;
mod terminal;
mod theme;

pub use audit_transcript::AuditTranscriptRenderer;
pub use compiler_text::CompilerTextRenderer;
pub use github_actions::GithubActionsRenderer;
pub use github_actions_delta::GithubActionsDeltaRenderer;

#[cfg(feature = "html")]
pub use html::{HtmlRenderer, HtmlSourceOptions, HtmlTheme};

pub use json::JsonRenderer;
pub use markdown::{MarkdownRenderer, MarkdownSourceOptions};
pub use plain::PlainRenderer;

pub use rendered_artifact::{
    RENDERED_RECEIPT_V1_SCHEMA, RenderedArtifact, RenderedArtifactReceipt, RenderedFormat,
    RenderedSourceDescriptor, RenderedSourceMode,
};

pub use sarif::SarifRenderer;
pub use terminal::TerminalRenderer;
pub use theme::{SeverityTheme, Style, Theme};

use crate::Diagnostic;

pub trait Renderer {
    fn render(&self, diagnostic: &Diagnostic) -> String;
}

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

impl ReportRenderer for CompilerTextRenderer {
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

#[cfg(feature = "html")]
impl ReportRenderer for HtmlRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        HtmlRenderer::render_report(self, diagnostics)
    }
}

impl ReportRenderer for JsonRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        self.render_report_with_policy(diagnostics, &crate::ExportPolicy::default())
    }
}

impl ReportRenderer for SarifRenderer {
    fn render_report<'a>(&self, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> String {
        self.render_many(diagnostics)
    }
}
