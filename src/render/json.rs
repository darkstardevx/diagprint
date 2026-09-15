use super::Renderer;
use crate::{Diagnostic, ExportDiagnostic};

#[derive(Debug, Default, Clone, Copy)]
pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        let export = ExportDiagnostic::from(diagnostic);

        serde_json::to_string_pretty(&export).expect("diagnostic serialization failed")
    }
}
