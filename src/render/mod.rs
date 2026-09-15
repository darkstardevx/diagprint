mod json;mod markdown;mod plain;mod terminal;
pub use json::JsonRenderer;pub use markdown::MarkdownRenderer;pub use plain::PlainRenderer;pub use terminal::TerminalRenderer;
use crate::Diagnostic; pub trait Renderer{fn render(&self,d:&Diagnostic)->String;}
