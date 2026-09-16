use super::{
    AuditTranscriptRenderer, CompilerTextRenderer, MarkdownRenderer, MarkdownSourceOptions,
    PlainRenderer, ReportRenderer,
};

#[cfg(feature = "html")]
use super::{HtmlRenderer, HtmlSourceOptions};

use crate::{
    ArtifactDigest, ArtifactVerificationError, CanonicalizationError, DiagnosticReport,
    ReportDigest, SourceCache, SourceSnapshot,
};
use serde::Serialize;

/// Stable schema identifier for rendered-report receipts.
pub const RENDERED_RECEIPT_V1_SCHEMA: &str = "diagprint.report.rendered.receipt/v1";

/// External rendered-report format.
///
/// The semantic diagnostic report remains independent of this presentation
/// choice. Each format therefore shares the same [`ReportDigest`] while
/// retaining its own exact rendered [`ArtifactDigest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderedFormat {
    Html,
    Markdown,
    PlainText,
    CompilerText,
    AuditTranscript,
}

impl RenderedFormat {
    /// Stable external name for this format.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "markdown",
            Self::PlainText => "plain_text",
            Self::CompilerText => "compiler_text",
            Self::AuditTranscript => "audit_transcript",
        }
    }

    /// Schema describing the rendered artifact contents.
    pub const fn artifact_schema(self) -> &'static str {
        match self {
            Self::Html => "diagprint.report.html/v1",

            Self::Markdown => "diagprint.report.markdown/v1",

            Self::PlainText => "diagprint.report.text/v1",

            Self::CompilerText => "diagprint.report.compiler-text/v1",

            Self::AuditTranscript => "diagprint.report.audit/v1",
        }
    }

    /// Internet media type for this representation.
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",

            Self::Markdown => "text/markdown; charset=utf-8",

            Self::PlainText | Self::CompilerText | Self::AuditTranscript => {
                "text/plain; charset=utf-8"
            }
        }
    }

    /// Conventional filename extension without a leading dot.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "md",
            Self::PlainText => "txt",
            Self::CompilerText => "txt",
            Self::AuditTranscript => "audit",
        }
    }
}

/// Source state used while creating a rendered artifact.
///
/// Source contents themselves are never copied into the receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderedSourceMode {
    None,
    Cache,
    Snapshot,
}

/// Privacy-safe description of source embedding.
///
/// This records how source text was obtained, not the source contents or
/// source paths themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct RenderedSourceDescriptor {
    pub mode: RenderedSourceMode,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_lines: Option<usize>,
}

impl RenderedSourceDescriptor {
    pub const fn none() -> Self {
        Self {
            mode: RenderedSourceMode::None,
            context_lines: None,
        }
    }

    pub const fn cache(context_lines: usize) -> Self {
        Self {
            mode: RenderedSourceMode::Cache,
            context_lines: Some(context_lines),
        }
    }

    pub const fn snapshot(context_lines: usize) -> Self {
        Self {
            mode: RenderedSourceMode::Snapshot,
            context_lines: Some(context_lines),
        }
    }
}

/// Receipt for one exact rendered diagnostic report.
#[derive(Debug, Clone, Serialize)]
pub struct RenderedArtifactReceipt {
    pub schema: &'static str,

    pub artifact_schema: &'static str,
    pub media_type: &'static str,
    pub format: RenderedFormat,

    pub report: ReportDigest,

    pub artifact_digest: ArtifactDigest,
    pub byte_length: usize,

    pub source: RenderedSourceDescriptor,
}

impl RenderedArtifactReceipt {
    /// Verifies arbitrary bytes against this receipt.
    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), ArtifactVerificationError> {
        if bytes.len() != self.byte_length {
            return Err(ArtifactVerificationError::Length {
                expected: self.byte_length,
                actual: bytes.len(),
            });
        }

        let actual = ArtifactDigest::compute(bytes);

        if actual != self.artifact_digest {
            return Err(ArtifactVerificationError::Digest {
                expected: self.artifact_digest,
                actual,
            });
        }

        Ok(())
    }
}

/// Exact rendered report bytes plus their integrity receipt.
#[derive(Debug, Clone)]
pub struct RenderedArtifact {
    bytes: Vec<u8>,
    receipt: RenderedArtifactReceipt,
}

impl RenderedArtifact {
    pub(super) fn new(
        report: &DiagnosticReport,
        format: RenderedFormat,
        rendered: String,
        source: RenderedSourceDescriptor,
    ) -> Result<Self, CanonicalizationError> {
        let report_digest = report.digest()?;

        Ok(Self::new_with_report_digest(
            report_digest,
            format,
            rendered,
            source,
        ))
    }

    fn new_with_report_digest(
        report_digest: ReportDigest,
        format: RenderedFormat,
        rendered: String,
        source: RenderedSourceDescriptor,
    ) -> Self {
        let bytes = rendered.into_bytes();

        let receipt = RenderedArtifactReceipt {
            schema: RENDERED_RECEIPT_V1_SCHEMA,

            artifact_schema: format.artifact_schema(),
            media_type: format.media_type(),
            format,

            report: report_digest,

            artifact_digest: ArtifactDigest::compute(&bytes),
            byte_length: bytes.len(),

            source,
        };

        Self { bytes, receipt }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn receipt(&self) -> &RenderedArtifactReceipt {
        &self.receipt
    }

    pub const fn format(&self) -> RenderedFormat {
        self.receipt.format
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.bytes)
    }

    pub fn verify(&self) -> Result<(), ArtifactVerificationError> {
        self.receipt.verify_bytes(&self.bytes)
    }
}

impl PlainRenderer {
    /// Renders a verified plain-text report artifact.
    pub fn render_report_artifact(
        &self,
        report: &DiagnosticReport,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let rendered = ReportRenderer::render_report(self, report.iter());

        RenderedArtifact::new(
            report,
            RenderedFormat::PlainText,
            rendered,
            RenderedSourceDescriptor::none(),
        )
    }
}

impl CompilerTextRenderer {
    /// Renders a verified compiler/editor-text report artifact.
    pub fn render_report_artifact(
        &self,
        report: &DiagnosticReport,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let rendered = ReportRenderer::render_report(self, report.iter());

        RenderedArtifact::new(
            report,
            RenderedFormat::CompilerText,
            rendered,
            RenderedSourceDescriptor::none(),
        )
    }
}

impl AuditTranscriptRenderer {
    /// Renders a verified deterministic audit-transcript artifact.
    pub fn render_report_artifact(
        &self,
        report: &DiagnosticReport,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let report_digest = report.digest()?;

        let rendered = self.render_report_with_digest(report, report_digest);

        Ok(RenderedArtifact::new_with_report_digest(
            report_digest,
            RenderedFormat::AuditTranscript,
            rendered,
            RenderedSourceDescriptor::none(),
        ))
    }
}

impl MarkdownRenderer {
    /// Renders a verified Markdown artifact without source contents.
    pub fn render_report_artifact(
        &self,
        report: &DiagnosticReport,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let rendered = ReportRenderer::render_report(self, report.iter());

        RenderedArtifact::new(
            report,
            RenderedFormat::Markdown,
            rendered,
            RenderedSourceDescriptor::none(),
        )
    }

    /// Renders a verified Markdown artifact with live cached source.
    pub fn render_report_artifact_with_sources(
        &self,
        report: &DiagnosticReport,
        sources: &SourceCache,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let options = MarkdownSourceOptions::default();

        let rendered = self.render_report_with_sources(report.iter(), sources);

        RenderedArtifact::new(
            report,
            RenderedFormat::Markdown,
            rendered,
            RenderedSourceDescriptor::cache(options.context_lines()),
        )
    }

    /// Renders a verified Markdown artifact against a source snapshot.
    pub fn render_report_artifact_with_snapshot(
        &self,
        report: &DiagnosticReport,
        sources: &SourceSnapshot,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let options = MarkdownSourceOptions::default();

        let rendered = self.render_report_with_snapshot(report.iter(), sources);

        RenderedArtifact::new(
            report,
            RenderedFormat::Markdown,
            rendered,
            RenderedSourceDescriptor::snapshot(options.context_lines()),
        )
    }
}

#[cfg(feature = "html")]
impl HtmlRenderer {
    /// Renders a verified standalone HTML artifact without source contents.
    pub fn render_report_artifact(
        &self,
        report: &DiagnosticReport,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let rendered = self.render_report(report.iter());

        RenderedArtifact::new(
            report,
            RenderedFormat::Html,
            rendered,
            RenderedSourceDescriptor::none(),
        )
    }

    /// Renders a verified standalone HTML artifact with live cached source.
    pub fn render_report_artifact_with_sources(
        &self,
        report: &DiagnosticReport,
        sources: &SourceCache,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let options = HtmlSourceOptions::default();

        let rendered = self.render_report_with_sources(report.iter(), sources);

        RenderedArtifact::new(
            report,
            RenderedFormat::Html,
            rendered,
            RenderedSourceDescriptor::cache(options.context_lines()),
        )
    }

    /// Renders a verified standalone HTML artifact against a source snapshot.
    pub fn render_report_artifact_with_snapshot(
        &self,
        report: &DiagnosticReport,
        sources: &SourceSnapshot,
    ) -> Result<RenderedArtifact, CanonicalizationError> {
        let options = HtmlSourceOptions::default();

        let rendered = self.render_report_with_snapshot(report.iter(), sources);

        RenderedArtifact::new(
            report,
            RenderedFormat::Html,
            rendered,
            RenderedSourceDescriptor::snapshot(options.context_lines()),
        )
    }
}
