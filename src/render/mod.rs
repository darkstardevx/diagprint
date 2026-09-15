mod json;
mod markdown;
mod plain;
mod terminal;
use crate::Diagnostic;
pub use json::JsonRenderer;
pub use markdown::MarkdownRenderer;
pub use plain::PlainRenderer;
pub use terminal::TerminalRenderer;
pub trait Renderer {
    fn render(&self, d: &Diagnostic) -> String;
}
