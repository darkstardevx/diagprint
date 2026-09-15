use crate::{Diagnostic, DiagnosticReport, render::Renderer};
use std::{
    error::Error,
    fmt,
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::Path,
    sync::Mutex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkErrorKind {
    Io,
    Serialization,
    Poisoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkError {
    kind: SinkErrorKind,
    message: String,
}

impl SinkError {
    pub fn new(kind: SinkErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> SinkErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn poisoned() -> Self {
        Self::new(SinkErrorKind::Poisoned, "diagnostic sink lock poisoned")
    }
}

impl fmt::Display for SinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SinkError {}

impl From<io::Error> for SinkError {
    fn from(error: io::Error) -> Self {
        Self::new(SinkErrorKind::Io, error.to_string())
    }
}

impl From<serde_json::Error> for SinkError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(SinkErrorKind::Serialization, error.to_string())
    }
}

pub type SinkResult<T> = Result<T, SinkError>;

/// Synchronous destination for structured diagnostics.
///
/// The core trait is intentionally runtime-agnostic. Networked, queued, and
/// asynchronous delivery belongs in companion crates such as
/// `diagprint-async`.
pub trait DiagnosticSink: Send + Sync {
    /// Delivers one diagnostic.
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()>;

    /// Delivers every diagnostic in a report.
    ///
    /// Returns the number successfully emitted before completion.
    fn emit_report(&self, report: &DiagnosticReport) -> SinkResult<usize> {
        let mut emitted = 0;

        for diagnostic in report {
            self.emit(diagnostic)?;
            emitted += 1;
        }

        Ok(emitted)
    }

    /// Flushes buffered output.
    fn flush(&self) -> SinkResult<()> {
        Ok(())
    }
}

/// Sink which renders diagnostics into an arbitrary synchronous writer.
pub struct WriterSink<W, R> {
    writer: Mutex<W>,
    renderer: R,
}

impl<W, R> WriterSink<W, R> {
    pub fn new(writer: W, renderer: R) -> Self {
        Self {
            writer: Mutex::new(writer),
            renderer,
        }
    }

    pub fn into_inner(self) -> SinkResult<W> {
        self.writer.into_inner().map_err(|_| SinkError::poisoned())
    }
}

impl<R> WriterSink<BufWriter<File>, R> {
    pub fn create_file(path: impl AsRef<Path>, renderer: R) -> SinkResult<Self> {
        let file = File::create(path)?;

        Ok(Self::new(BufWriter::new(file), renderer))
    }

    pub fn append_file(path: impl AsRef<Path>, renderer: R) -> SinkResult<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self::new(BufWriter::new(file), renderer))
    }
}

impl<W, R> DiagnosticSink for WriterSink<W, R>
where
    W: Write + Send,
    R: Renderer + Send + Sync,
{
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        let rendered = self.renderer.render(diagnostic);

        let mut writer = self.writer.lock().map_err(|_| SinkError::poisoned())?;

        writer.write_all(rendered.as_bytes())?;

        writer.write_all(b"\n")?;

        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        let mut writer = self.writer.lock().map_err(|_| SinkError::poisoned())?;

        writer.flush()?;

        Ok(())
    }
}

/// Newline-delimited JSON sink suitable for logs and stream processing.
///
/// Each diagnostic is serialized as one compact JSON object followed by a
/// newline.
pub struct JsonLinesSink<W> {
    writer: Mutex<W>,
}

impl<W> JsonLinesSink<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
        }
    }

    pub fn into_inner(self) -> SinkResult<W> {
        self.writer.into_inner().map_err(|_| SinkError::poisoned())
    }
}

impl JsonLinesSink<BufWriter<File>> {
    pub fn create_file(path: impl AsRef<Path>) -> SinkResult<Self> {
        let file = File::create(path)?;

        Ok(Self::new(BufWriter::new(file)))
    }

    pub fn append_file(path: impl AsRef<Path>) -> SinkResult<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self::new(BufWriter::new(file)))
    }
}

impl<W> DiagnosticSink for JsonLinesSink<W>
where
    W: Write + Send,
{
    fn emit(&self, diagnostic: &Diagnostic) -> SinkResult<()> {
        let mut writer = self.writer.lock().map_err(|_| SinkError::poisoned())?;

        serde_json::to_writer(&mut *writer, diagnostic)?;

        writer.write_all(b"\n")?;

        Ok(())
    }

    fn flush(&self) -> SinkResult<()> {
        let mut writer = self.writer.lock().map_err(|_| SinkError::poisoned())?;

        writer.flush()?;

        Ok(())
    }
}
