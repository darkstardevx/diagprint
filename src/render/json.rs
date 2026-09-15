use super::Renderer;
use crate::{Diagnostic, ExportDiagnostic, ExportPolicy};

#[derive(Debug, Default, Clone, Copy)]
pub struct JsonRenderer;

impl JsonRenderer {
    /// Renders one diagnostic using an explicit external-export policy.
    pub fn render_with_policy(&self, diagnostic: &Diagnostic, policy: &ExportPolicy) -> String {
        let export = ExportDiagnostic::with_policy(diagnostic, policy);

        serde_json::to_string_pretty(&export).expect("diagnostic serialization failed")
    }

    /// Renders multiple diagnostics as one JSON array using an explicit
    /// external-export policy.
    pub fn render_report_with_policy<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        policy: &ExportPolicy,
    ) -> String {
        let diagnostics = diagnostics
            .into_iter()
            .map(|diagnostic| ExportDiagnostic::with_policy(diagnostic, policy))
            .collect::<Vec<_>>();

        serde_json::to_string_pretty(&diagnostics).expect("diagnostic report serialization failed")
    }
}

impl Renderer for JsonRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        self.render_with_policy(diagnostic, &ExportPolicy::default())
    }
}
