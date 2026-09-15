use crate::{
    CapturedDiagnostic, Diagnostic, DiagnosticReport, Severity, SourceCache, SourceProvider,
    SourceSnapshot,
    render::{
        GithubActionsRenderer, JsonRenderer, MarkdownRenderer, PlainRenderer, Renderer,
        ReportRenderer, SarifRenderer, TerminalRenderer, Theme,
    },
    rotation::{RotationCadence, RotationPolicy, RotationState},
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Gzip,
    Zstd,
}

#[derive(Debug, Clone)]
pub struct Reporter {
    application: String,
    session_id: Uuid,
    min_severity: Severity,
    file: Option<PathBuf>,
    terminal: TerminalRenderer,
    source_cache: SourceCache,
    rotation: RotationState,
    lock: Arc<Mutex<()>>,
    compression: Compression,
}

impl Reporter {
    pub fn builder() -> ReporterBuilder {
        ReporterBuilder::default()
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Returns a shared handle to this reporter's source cache.
    ///
    /// Cloning the returned handle does not copy source text.
    pub fn source_cache(&self) -> SourceCache {
        self.source_cache.clone()
    }

    /// Captures the reporter's current in-memory sources.
    ///
    /// Later changes to the live source cache do not affect the returned
    /// snapshot.
    pub fn source_snapshot(&self) -> SourceSnapshot {
        self.source_cache.snapshot()
    }

    /// Captures a diagnostic together with the reporter's current source
    /// snapshot.
    ///
    /// Any unversioned labels whose sources are present in the reporter cache
    /// are bound to the revisions captured by that snapshot.
    pub fn capture(&self, diagnostic: Diagnostic) -> CapturedDiagnostic {
        CapturedDiagnostic::new(diagnostic, self.source_snapshot())
    }

    /// Inserts or replaces an in-memory source available to terminal rendering.
    pub fn register_source(&self, name: impl Into<String>, source: impl Into<String>) {
        self.source_cache.insert(name, source);
    }

    /// Registers every source currently exposed by a source provider.
    pub fn register_sources(&self, provider: &impl SourceProvider) {
        provider.populate_source_cache(&self.source_cache);
    }

    /// Removes an in-memory source from this reporter.
    pub fn remove_source(&self, name: &str) -> bool {
        self.source_cache.remove(name).is_some()
    }

    pub fn diagnostic(&self, severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.session_id, self.application.clone(), severity, message)
    }

    pub fn trace(&self, message: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Trace, message)
    }

    pub fn debug(&self, message: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Debug, message)
    }

    pub fn info(&self, message: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Info, message)
    }

    pub fn warning(&self, message: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Warning, message)
    }

    pub fn error(&self, message: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Error, message)
    }

    pub fn fatal(&self, message: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Fatal, message)
    }

    pub fn emit(&self, diagnostic: &Diagnostic) -> io::Result<bool> {
        if diagnostic.severity < self.min_severity {
            return Ok(false);
        }

        print!(
            "{}",
            self.terminal
                .render_with_sources(diagnostic, &self.source_cache)
        );
        io::stdout().flush()?;

        if let Some(path) = &self.file {
            self.write(path, &PlainRenderer.render(diagnostic))?;
        }

        Ok(true)
    }

    /// Emits a diagnostic against an immutable source snapshot.
    ///
    /// This is useful for diagnostics created from editor buffers or other
    /// mutable in-memory sources whose live contents may have changed since
    /// the diagnostic was produced.
    pub fn emit_with_snapshot(
        &self,
        diagnostic: &Diagnostic,
        sources: &SourceSnapshot,
    ) -> io::Result<bool> {
        if diagnostic.severity < self.min_severity {
            return Ok(false);
        }

        print!(
            "{}",
            self.terminal.render_with_snapshot(diagnostic, sources)
        );

        io::stdout().flush()?;

        if let Some(path) = &self.file {
            self.write(path, &PlainRenderer.render(diagnostic))?;
        }

        Ok(true)
    }

    /// Emits a captured diagnostic using its immutable source snapshot.
    pub fn emit_captured(&self, captured: &CapturedDiagnostic) -> io::Result<bool> {
        self.emit_with_snapshot(captured.diagnostic(), captured.sources())
    }

    /// Emits every diagnostic in a report using the normal terminal renderer.
    ///
    /// Returns the number of diagnostics which passed the reporter's minimum
    /// severity filter.
    pub fn emit_report(&self, report: &DiagnosticReport) -> io::Result<usize> {
        let mut emitted = 0;

        for diagnostic in report {
            if self.emit(diagnostic)? {
                emitted += 1;
            }
        }

        Ok(emitted)
    }

    /// Emits every diagnostic in a report against one immutable source
    /// snapshot.
    pub fn emit_report_with_snapshot(
        &self,
        report: &DiagnosticReport,
        sources: &SourceSnapshot,
    ) -> io::Result<usize> {
        let mut emitted = 0;

        for diagnostic in report {
            if self.emit_with_snapshot(diagnostic, sources)? {
                emitted += 1;
            }
        }

        Ok(emitted)
    }

    /// Emits one valid GitHub Actions batch.
    pub fn emit_github_actions_report(&self, report: &DiagnosticReport) -> io::Result<usize> {
        self.emit_rendered_report(report, &GithubActionsRenderer)
    }

    /// Emits one JSON array containing every diagnostic which passes the
    /// reporter's minimum severity filter.
    pub fn emit_json_report(&self, report: &DiagnosticReport) -> io::Result<usize> {
        self.emit_rendered_report(report, &JsonRenderer)
    }

    /// Emits one Markdown document containing the filtered report.
    pub fn emit_markdown_report(&self, report: &DiagnosticReport) -> io::Result<usize> {
        self.emit_rendered_report(report, &MarkdownRenderer)
    }

    /// Emits one complete SARIF 2.1.0 document containing the filtered report.
    pub fn emit_sarif_report(&self, report: &DiagnosticReport) -> io::Result<usize> {
        self.emit_rendered_report(report, &SarifRenderer)
    }

    /// Emits GitHub Actions workflow-command annotations.
    pub fn emit_github_actions(&self, diagnostic: &Diagnostic) -> io::Result<bool> {
        self.emit_with(diagnostic, &GithubActionsRenderer)
    }

    pub fn emit_json(&self, diagnostic: &Diagnostic) -> io::Result<bool> {
        self.emit_with(diagnostic, &JsonRenderer)
    }

    pub fn emit_markdown(&self, diagnostic: &Diagnostic) -> io::Result<bool> {
        self.emit_with(diagnostic, &MarkdownRenderer)
    }

    fn emit_rendered_report(
        &self,
        report: &DiagnosticReport,
        renderer: &impl ReportRenderer,
    ) -> io::Result<usize> {
        let diagnostics = report
            .iter()
            .filter(|diagnostic| diagnostic.severity >= self.min_severity)
            .collect::<Vec<_>>();

        if diagnostics.is_empty() {
            return Ok(0);
        }

        let rendered = renderer.render_report(diagnostics.iter().copied());

        println!("{rendered}");

        if let Some(path) = &self.file {
            self.write(path, &rendered)?;
        }

        Ok(diagnostics.len())
    }

    fn emit_with(&self, diagnostic: &Diagnostic, renderer: &impl Renderer) -> io::Result<bool> {
        if diagnostic.severity < self.min_severity {
            return Ok(false);
        }

        let rendered = renderer.render(diagnostic);

        println!("{rendered}");

        if let Some(path) = &self.file {
            self.write(path, &rendered)?;
        }

        Ok(true)
    }

    fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| io::Error::other("diagprint output lock poisoned"))?;

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        if self.rotation.should_rotate(path, text.len() + 1)? {
            if let Some(archive) = self.rotation.rotate(path)? {
                self.compress(&archive)?;
            }
        }

        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{text}")
    }

    fn compress(&self, path: &Path) -> io::Result<()> {
        match self.compression {
            Compression::None => Ok(()),
            Compression::Gzip => compress_gzip(path),
            Compression::Zstd => compress_zstd(path),
        }
    }
}

#[cfg(feature = "compression")]
fn compress_gzip(path: &Path) -> io::Result<()> {
    use flate2::{Compression as GzipCompression, write::GzEncoder};

    let input = fs::read(path)?;
    let output = fs::File::create(format!("{}.gz", path.display()))?;

    let mut encoder = GzEncoder::new(output, GzipCompression::default());
    encoder.write_all(&input)?;
    encoder.finish()?;

    fs::remove_file(path)
}

#[cfg(not(feature = "compression"))]
fn compress_gzip(_: &Path) -> io::Result<()> {
    Err(io::Error::other(
        "enable diagprint feature `compression` for gzip",
    ))
}

#[cfg(feature = "compression")]
fn compress_zstd(path: &Path) -> io::Result<()> {
    let input = fs::read(path)?;
    let output = fs::File::create(format!("{}.zst", path.display()))?;

    zstd::stream::copy_encode(&input[..], output, 3)?;

    fs::remove_file(path)
}

#[cfg(not(feature = "compression"))]
fn compress_zstd(_: &Path) -> io::Result<()> {
    Err(io::Error::other(
        "enable diagprint feature `compression` for zstd",
    ))
}

#[derive(Debug, Clone)]
pub struct ReporterBuilder {
    application: String,
    min_severity: Severity,
    file: Option<PathBuf>,
    color: bool,
    show_metadata: bool,
    source_context_lines: usize,
    width: usize,
    max_file_size: Option<u64>,
    rotation_count: usize,
    cadence: RotationCadence,
    compression: Compression,
    theme: Theme,
    source_cache: SourceCache,
}

impl Default for ReporterBuilder {
    fn default() -> Self {
        Self {
            application: "application".into(),
            min_severity: Severity::Trace,
            file: None,
            color: true,
            show_metadata: false,
            source_context_lines: 1,
            width: 72,
            max_file_size: None,
            rotation_count: 5,
            cadence: RotationCadence::Never,
            compression: Compression::None,
            theme: Theme::default(),
            source_cache: SourceCache::new(),
        }
    }
}

impl ReporterBuilder {
    pub fn application(mut self, value: impl Into<String>) -> Self {
        self.application = value.into();
        self
    }

    pub fn min_severity(mut self, value: Severity) -> Self {
        self.min_severity = value;
        self
    }

    pub fn file(mut self, value: impl Into<PathBuf>) -> Self {
        self.file = Some(value.into());
        self
    }

    pub fn color(mut self, value: bool) -> Self {
        self.color = value;
        self
    }

    pub fn show_metadata(mut self, value: bool) -> Self {
        self.show_metadata = value;
        self
    }

    pub fn source_context_lines(mut self, value: usize) -> Self {
        self.source_context_lines = value;
        self
    }

    pub fn width(mut self, value: usize) -> Self {
        self.width = value;
        self
    }

    pub fn max_file_size(mut self, value: u64) -> Self {
        self.max_file_size = Some(value);
        self
    }

    pub fn rotation_count(mut self, value: usize) -> Self {
        self.rotation_count = value;
        self
    }

    pub fn rotation_cadence(mut self, value: RotationCadence) -> Self {
        self.cadence = value;
        self
    }

    pub fn compression(mut self, value: Compression) -> Self {
        self.compression = value;
        self
    }

    pub fn theme(mut self, value: Theme) -> Self {
        self.theme = value;
        self
    }

    /// Registers an in-memory source before the reporter is built.
    pub fn source(self, name: impl Into<String>, source: impl Into<String>) -> Self {
        self.source_cache.insert(name, source);
        self
    }

    /// Uses an existing shared source cache.
    pub fn source_cache(mut self, value: SourceCache) -> Self {
        self.source_cache = value;
        self
    }

    /// Adds all sources currently exposed by a source provider.
    pub fn sources_from(self, provider: &impl SourceProvider) -> Self {
        provider.populate_source_cache(&self.source_cache);
        self
    }

    pub fn build(self) -> io::Result<Reporter> {
        Ok(Reporter {
            application: self.application,
            session_id: Uuid::now_v7(),
            min_severity: self.min_severity,
            file: self.file,
            terminal: TerminalRenderer {
                color: self.color,
                show_metadata: self.show_metadata,
                source_context_lines: self.source_context_lines,
                width: self.width,
                theme: self.theme,
            },
            source_cache: self.source_cache,
            rotation: RotationState::new(RotationPolicy {
                max_file_size: self.max_file_size,
                max_files: self.rotation_count,
                cadence: self.cadence,
            }),
            lock: Arc::new(Mutex::new(())),
            compression: self.compression,
        })
    }
}
