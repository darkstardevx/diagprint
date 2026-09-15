use crate::{
    render::{JsonRenderer, MarkdownRenderer, PlainRenderer, Renderer, TerminalRenderer},
    rotation::{RotationCadence, RotationPolicy, RotationState},
    Diagnostic, Severity,
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
    pub fn diagnostic(&self, s: Severity, m: impl Into<String>) -> Diagnostic {
        Diagnostic::new(self.session_id, self.application.clone(), s, m)
    }
    pub fn trace(&self, m: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Trace, m)
    }
    pub fn debug(&self, m: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Debug, m)
    }
    pub fn info(&self, m: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Info, m)
    }
    pub fn warning(&self, m: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Warning, m)
    }
    pub fn error(&self, m: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Error, m)
    }
    pub fn fatal(&self, m: impl Into<String>) -> Diagnostic {
        self.diagnostic(Severity::Fatal, m)
    }
    pub fn emit(&self, d: &Diagnostic) -> io::Result<bool> {
        if d.severity < self.min_severity {
            return Ok(false);
        }
        print!("{}", self.terminal.render(d));
        io::stdout().flush()?;
        if let Some(p) = &self.file {
            self.write(p, &PlainRenderer.render(d))?
        }
        Ok(true)
    }
    pub fn emit_json(&self, d: &Diagnostic) -> io::Result<bool> {
        self.emit_with(d, &JsonRenderer)
    }
    pub fn emit_markdown(&self, d: &Diagnostic) -> io::Result<bool> {
        self.emit_with(d, &MarkdownRenderer)
    }
    fn emit_with(&self, d: &Diagnostic, r: &impl Renderer) -> io::Result<bool> {
        if d.severity < self.min_severity {
            return Ok(false);
        }
        let t = r.render(d);
        println!("{t}");
        if let Some(p) = &self.file {
            self.write(p, &t)?
        }
        Ok(true)
    }
    fn write(&self, p: &Path, t: &str) -> io::Result<()> {
        let _g = self
            .lock
            .lock()
            .map_err(|_| io::Error::other("diagprint output lock poisoned"))?;
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?
            }
        }
        if self.rotation.should_rotate(p, t.len() + 1)? {
            if let Some(a) = self.rotation.rotate(p)? {
                self.compress(&a)?
            }
        }
        let mut f = OpenOptions::new().create(true).append(true).open(p)?;
        writeln!(f, "{t}")
    }
    fn compress(&self, p: &Path) -> io::Result<()> {
        match self.compression {
            Compression::None => Ok(()),
            Compression::Gzip => compress_gzip(p),
            Compression::Zstd => compress_zstd(p),
        }
    }
}
#[cfg(feature = "compression")]
fn compress_gzip(p: &Path) -> io::Result<()> {
    use flate2::{write::GzEncoder, Compression as Gz};
    let input = fs::read(p)?;
    let out = fs::File::create(format!("{}.gz", p.display()))?;
    let mut enc = GzEncoder::new(out, Gz::default());
    enc.write_all(&input)?;
    enc.finish()?;
    fs::remove_file(p)
}
#[cfg(not(feature = "compression"))]
fn compress_gzip(_: &Path) -> io::Result<()> {
    Err(io::Error::other(
        "enable diagprint feature `compression` for gzip",
    ))
}
#[cfg(feature = "compression")]
fn compress_zstd(p: &Path) -> io::Result<()> {
    let input = fs::read(p)?;
    let out = fs::File::create(format!("{}.zst", p.display()))?;
    zstd::stream::copy_encode(&input[..], out, 3)?;
    fs::remove_file(p)
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
        }
    }
}
impl ReporterBuilder {
    pub fn application(mut self, v: impl Into<String>) -> Self {
        self.application = v.into();
        self
    }
    pub fn min_severity(mut self, v: Severity) -> Self {
        self.min_severity = v;
        self
    }
    pub fn file(mut self, v: impl Into<PathBuf>) -> Self {
        self.file = Some(v.into());
        self
    }
    pub fn color(mut self, v: bool) -> Self {
        self.color = v;
        self
    }
    pub fn show_metadata(mut self, v: bool) -> Self {
        self.show_metadata = v;
        self
    }
    pub fn source_context_lines(mut self, v: usize) -> Self {
        self.source_context_lines = v;
        self
    }
    pub fn width(mut self, v: usize) -> Self {
        self.width = v;
        self
    }
    pub fn max_file_size(mut self, v: u64) -> Self {
        self.max_file_size = Some(v);
        self
    }
    pub fn rotation_count(mut self, v: usize) -> Self {
        self.rotation_count = v;
        self
    }
    pub fn rotation_cadence(mut self, v: RotationCadence) -> Self {
        self.cadence = v;
        self
    }
    pub fn compression(mut self, v: Compression) -> Self {
        self.compression = v;
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
            },
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
