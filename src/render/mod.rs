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

use crate::Diagnostic;

pub trait Renderer {
    fn render(&self, diagnostic: &Diagnostic) -> String;
}
