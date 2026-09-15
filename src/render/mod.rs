mod json;
mod markdown;
mod plain;
mod terminal;
mod theme;

pub use json::JsonRenderer;
pub use markdown::MarkdownRenderer;
pub use plain::PlainRenderer;
pub use terminal::TerminalRenderer;
pub use theme::{SeverityTheme, Style, Theme};

use crate::Diagnostic;

pub trait Renderer {
    fn render(&self, diagnostic: &Diagnostic) -> String;
}
