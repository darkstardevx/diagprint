//! Import structured diagnostics emitted by rustc and Cargo.
//!
//! rustc can emit one JSON diagnostic per line with
//! `--error-format=json`. Cargo wraps rustc diagnostics in
//! `reason = "compiler-message"` messages when using
//! `--message-format=json`.
//!
//! The importer deliberately treats compiler output as data rather than
//! scraping rustc's human-readable rendering.

use crate::{
    Applicability, Diagnostic, DocumentationResolver, Edit, Reporter, Severity, Suggestion,
    TextRange,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum CompilerImportError {
    Json(serde_json::Error),
}

impl fmt::Display for CompilerImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => {
                write!(f, "invalid compiler diagnostic JSON: {error}")
            }
        }
    }
}

impl Error for CompilerImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
        }
    }
}

impl From<serde_json::Error> for CompilerImportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompilerImporter {
    source_root: Option<PathBuf>,

    hydrate_edits: bool,

    documentation: DocumentationResolver,
}

impl CompilerImporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.source_root = Some(root.into());

        self
    }

    pub fn hydrate_edits(mut self, enabled: bool) -> Self {
        self.hydrate_edits = enabled;

        self
    }

    /// Configures the documentation policy used for compiler-generated links.
    pub fn documentation_resolver(mut self, resolver: DocumentationResolver) -> Self {
        self.documentation = resolver;

        self
    }

    pub fn import_line(
        &self,
        reporter: &Reporter,
        line: &str,
    ) -> Result<Option<Diagnostic>, CompilerImportError> {
        let line = line.trim();

        if line.is_empty() || !line.starts_with('{') {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(line)?;

        if value.get("reason").and_then(Value::as_str) == Some("compiler-message") {
            let envelope: CargoCompilerMessage = serde_json::from_value(value)?;

            return Ok(Some(self.convert_cargo(reporter, envelope)));
        }

        let is_rustc_diagnostic =
            value.get("$message_type").and_then(Value::as_str) == Some("diagnostic");

        let looks_like_diagnostic = value.get("message").is_some_and(Value::is_string)
            && value.get("level").is_some_and(Value::is_string);

        if is_rustc_diagnostic || looks_like_diagnostic {
            let diagnostic: RustcDiagnostic = serde_json::from_value(value)?;

            return Ok(Some(self.convert(reporter, diagnostic)));
        }

        Ok(None)
    }

    fn convert_cargo(&self, reporter: &Reporter, envelope: CargoCompilerMessage) -> Diagnostic {
        let CargoCompilerMessage {
            package_id,
            manifest_path,
            target,
            message,
        } = envelope;

        let mut diagnostic = self.convert(reporter, message);

        if let Some(package_id) = package_id {
            diagnostic = diagnostic.note(format!("cargo package: {package_id}"));
        }

        if let Some(target) = target {
            if let Some(name) = target.name {
                diagnostic = diagnostic.note(format!("cargo target: {name}"));
            }
        }

        if let Some(manifest_path) = manifest_path {
            diagnostic = diagnostic.note(format!("cargo manifest: {}", manifest_path.display()));
        }

        diagnostic
    }

    fn convert(&self, reporter: &Reporter, raw: RustcDiagnostic) -> Diagnostic {
        let RustcDiagnostic {
            message,
            code,
            level,
            spans,
            children,
        } = raw;

        let mut diagnostic = reporter.diagnostic(severity_from_rustc(&level), message);

        if let Some(code) = &code {
            diagnostic = diagnostic.code(code.code.clone());
        }

        diagnostic = self.attach_primary_spans(diagnostic, &spans);

        let mut has_help = false;

        for child in children {
            if let Some(suggestion) = self.compiler_suggestion(&child) {
                diagnostic = diagnostic.suggestion(suggestion);

                continue;
            }

            match child.level.as_str() {
                "help" if !has_help => {
                    diagnostic = diagnostic.help(child.message);

                    has_help = true;
                }

                "help" => {
                    diagnostic = diagnostic.note(format!("rustc help: {}", child.message));
                }

                "note" | "failure-note" => {
                    diagnostic = diagnostic.note(child.message);
                }

                _ => {
                    diagnostic =
                        diagnostic.note(format!("rustc {}: {}", child.level, child.message));
                }
            }
        }

        if let Some(code) = code {
            if is_rust_error_code(&code.code) {
                diagnostic = diagnostic.suggestion(
                    Suggestion::new(format!(
                        "Read Rust compiler documentation for {}",
                        code.code
                    ))
                    .explanation("The compiler supplied a documented Rust error code.")
                    .applicability(Applicability::Manual)
                    .documentation(self.documentation.rust_error(&code.code)),
                );
            }
        }

        diagnostic
    }

    fn attach_primary_spans(&self, mut diagnostic: Diagnostic, spans: &[RustcSpan]) -> Diagnostic {
        let primary: Vec<&RustcSpan> = spans.iter().filter(|span| span.is_primary).collect();

        let selected: Vec<&RustcSpan> = if primary.is_empty() {
            spans.first().into_iter().collect()
        } else {
            primary
        };

        for span in selected {
            let file = self.resolved_display_path(&span.file_name);

            let width = if span.line_start == span.line_end {
                let width = span.column_end.saturating_sub(span.column_start);

                (width > 0).then_some(width as usize)
            } else {
                None
            };

            diagnostic = diagnostic.label(
                file.to_string_lossy().into_owned(),
                span.line_start,
                Some(span.column_start),
                width,
                span.label.clone(),
            );
        }

        diagnostic
    }

    fn compiler_suggestion(&self, child: &RustcDiagnostic) -> Option<Suggestion> {
        let replacement_spans: Vec<&RustcSpan> = child
            .spans
            .iter()
            .filter(|span| span.suggested_replacement.is_some())
            .collect();

        if replacement_spans.is_empty() {
            return None;
        }

        let mut applicability = combined_applicability(&replacement_spans);

        let mut edits = Vec::new();

        let mut hydration_complete = true;

        for span in replacement_spans {
            match self.hydrate_span(span) {
                Some(hydrated) => {
                    if !hydrated.safe_for_automatic_apply
                        && applicability == Applicability::MachineApplicable
                    {
                        applicability = Applicability::MaybeIncorrect;
                    }

                    edits.push(hydrated.edit);
                }

                None => {
                    hydration_complete = false;
                }
            }
        }

        if !hydration_complete || edits.is_empty() {
            applicability = Applicability::Manual;
        }

        let mut suggestion = Suggestion::new(child.message.clone())
            .explanation("Imported from a structured rustc suggestion.")
            .applicability(applicability);

        for edit in edits {
            suggestion = suggestion.edit(edit);
        }

        suggestion = suggestion.documentation(self.documentation.rustc_json());

        Some(suggestion)
    }

    fn hydrate_span(&self, span: &RustcSpan) -> Option<HydratedEdit> {
        if !self.hydrate_edits {
            return None;
        }

        let root = self.source_root.as_ref()?;

        let root = fs::canonicalize(root).ok()?;

        let candidate = self.resolve_path(&span.file_name);

        let candidate = fs::canonicalize(candidate).ok()?;

        if !candidate.starts_with(&root) {
            return None;
        }

        let content = fs::read_to_string(&candidate).ok()?;

        if span.byte_start > span.byte_end || span.byte_end > content.len() {
            return None;
        }

        if !content.is_char_boundary(span.byte_start) || !content.is_char_boundary(span.byte_end) {
            return None;
        }

        let replacement = span.suggested_replacement.clone()?;

        if span.byte_start == span.byte_end {
            let expected_before = insertion_context(&content, span.byte_start);

            let edit = match expected_before {
                Some(expected_before) => {
                    Edit::insert_after(candidate, span.byte_start, expected_before, replacement)
                }

                None => Edit::insert(candidate, span.byte_start, replacement),
            };

            return Some(HydratedEdit {
                edit,

                safe_for_automatic_apply: false,
            });
        }

        let expected = content[span.byte_start..span.byte_end].to_owned();

        let range = TextRange::new(span.byte_start, span.byte_end);

        let edit = if replacement.is_empty() {
            Edit::delete(candidate, range, expected)
        } else {
            Edit::replace(candidate, range, expected, replacement)
        };

        Some(HydratedEdit {
            edit,

            safe_for_automatic_apply: true,
        })
    }

    fn resolved_display_path(&self, file_name: &str) -> PathBuf {
        self.resolve_path(file_name)
    }

    fn resolve_path(&self, file_name: &str) -> PathBuf {
        let path = Path::new(file_name);

        if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(root) = &self.source_root {
            root.join(path)
        } else {
            path.to_path_buf()
        }
    }
}

#[derive(Debug)]
struct HydratedEdit {
    edit: Edit,

    safe_for_automatic_apply: bool,
}

#[derive(Debug, Deserialize)]
struct CargoCompilerMessage {
    #[serde(default)]
    package_id: Option<String>,

    #[serde(default)]
    manifest_path: Option<PathBuf>,

    #[serde(default)]
    target: Option<CargoTarget>,

    message: RustcDiagnostic,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RustcDiagnostic {
    message: String,

    #[serde(default)]
    code: Option<RustcCode>,

    level: String,

    #[serde(default)]
    spans: Vec<RustcSpan>,

    #[serde(default)]
    children: Vec<RustcDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct RustcCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct RustcSpan {
    file_name: String,

    byte_start: usize,
    byte_end: usize,

    line_start: u32,
    line_end: u32,

    column_start: u32,
    column_end: u32,

    #[serde(default)]
    is_primary: bool,

    #[serde(default)]
    label: Option<String>,

    #[serde(default)]
    suggested_replacement: Option<String>,

    #[serde(default)]
    suggestion_applicability: Option<String>,
}

fn severity_from_rustc(level: &str) -> Severity {
    match level {
        "error: internal compiler error" => Severity::Fatal,

        "error" => Severity::Error,

        "warning" => Severity::Warning,

        "note" | "help" | "failure-note" => Severity::Info,

        _ => Severity::Info,
    }
}

fn combined_applicability(spans: &[&RustcSpan]) -> Applicability {
    let values: Vec<Applicability> = spans
        .iter()
        .map(|span| applicability_from_rustc(span.suggestion_applicability.as_deref()))
        .collect();

    if values
        .iter()
        .all(|value| *value == Applicability::MachineApplicable)
    {
        return Applicability::MachineApplicable;
    }

    if values.contains(&Applicability::Manual) {
        return Applicability::Manual;
    }

    if values.contains(&Applicability::HasPlaceholders) {
        return Applicability::HasPlaceholders;
    }

    Applicability::MaybeIncorrect
}

fn applicability_from_rustc(value: Option<&str>) -> Applicability {
    match value {
        Some("MachineApplicable") => Applicability::MachineApplicable,

        Some("MaybeIncorrect") => Applicability::MaybeIncorrect,

        Some("HasPlaceholders") => Applicability::HasPlaceholders,

        _ => Applicability::Manual,
    }
}

fn is_rust_error_code(code: &str) -> bool {
    let bytes = code.as_bytes();

    bytes.len() == 5 && bytes[0] == b'E' && bytes[1..].iter().all(u8::is_ascii_digit)
}

fn insertion_context(content: &str, offset: usize) -> Option<String> {
    if offset == 0 {
        return None;
    }

    let mut start = offset.saturating_sub(64);

    while start < offset && !content.is_char_boundary(start) {
        start += 1;
    }

    let context = &content[start..offset];

    (!context.is_empty()).then(|| context.to_owned())
}
