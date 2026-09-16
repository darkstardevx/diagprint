use super::Renderer;
use crate::{Diagnostic, ExportDiagnostic, ExportPolicy};

const DIAGNOSTIC_SERIALIZATION_FALLBACK: &str = r#"{
  "diagprint_error": "diagnostic JSON serialization failed"
}"#;

const REPORT_SERIALIZATION_FALLBACK: &str = r#"[
  {
    "diagprint_error": "diagnostic report JSON serialization failed"
  }
]"#;

#[derive(Debug, Default, Clone, Copy)]
pub struct JsonRenderer;

impl JsonRenderer {
    /// Attempts to render one diagnostic using an explicit external-export
    /// policy.
    pub fn try_render_with_policy(
        &self,
        diagnostic: &Diagnostic,
        policy: &ExportPolicy,
    ) -> serde_json::Result<String> {
        let export = ExportDiagnostic::with_policy(diagnostic, policy);

        serde_json::to_string_pretty(&export)
    }

    /// Renders one diagnostic using an explicit external-export policy.
    ///
    /// This compatibility API never panics on serialization failure. Call
    /// [`JsonRenderer::try_render_with_policy`] when the caller needs the
    /// serialization error itself.
    pub fn render_with_policy(&self, diagnostic: &Diagnostic, policy: &ExportPolicy) -> String {
        self.try_render_with_policy(diagnostic, policy)
            .unwrap_or_else(|_| DIAGNOSTIC_SERIALIZATION_FALLBACK.to_owned())
    }

    /// Attempts to render multiple diagnostics as one JSON array using an
    /// explicit external-export policy.
    pub fn try_render_report_with_policy<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        policy: &ExportPolicy,
    ) -> serde_json::Result<String> {
        let diagnostics = diagnostics
            .into_iter()
            .map(|diagnostic| ExportDiagnostic::with_policy(diagnostic, policy))
            .collect::<Vec<_>>();

        serde_json::to_string_pretty(&diagnostics)
    }

    /// Renders multiple diagnostics as one JSON array using an explicit
    /// external-export policy.
    ///
    /// This compatibility API never panics on serialization failure. Call
    /// [`JsonRenderer::try_render_report_with_policy`] when the caller needs
    /// the serialization error itself.
    pub fn render_report_with_policy<'a>(
        &self,
        diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
        policy: &ExportPolicy,
    ) -> String {
        self.try_render_report_with_policy(diagnostics, policy)
            .unwrap_or_else(|_| REPORT_SERIALIZATION_FALLBACK.to_owned())
    }
}

impl Renderer for JsonRenderer {
    fn render(&self, diagnostic: &Diagnostic) -> String {
        self.render_with_policy(diagnostic, &ExportPolicy::default())
    }
}
