mod analyzers;

pub use analyzers::{
    CargoAnalyzer, ClippyAnalyzer, CompilerAnalyzer, ConfigurationAnalyzer, DependencyAnalyzer,
    DocumentationAnalyzer, GitAnalyzer, LicenseAnalyzer, ManifestAnalyzer, RemediationAnalyzer,
    SecurityAnalyzer, SourceInventoryAnalyzer, TestAnalyzer, UnsafeInventoryAnalyzer,
    WorkspaceAnalyzer,
};

use crate::{DiagnosticReport, Reporter};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

const MAX_TEXT_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Execution depth used by project-scanning frontends.
///
/// The core scanner never executes commands itself. Profiles tell a frontend
/// which external evidence it may choose to collect before running analyzers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProjectScanProfile {
    /// Filesystem, source, manifest, configuration, documentation, license,
    /// dependency, and security inspection only.
    Static,

    /// Static inspection plus non-build project metadata such as Cargo metadata
    /// and Git status.
    #[default]
    Standard,

    /// Full tool-assisted scanning. Frontends may additionally run Cargo check,
    /// Clippy, and tests before feeding their captured output into the scanner.
    ///
    /// Those tools may execute project-controlled build scripts, proc macros,
    /// and tests, so this profile should require explicit opt-in.
    Deep,
}

impl ProjectScanProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }
}

/// Broad classification used by built-in filesystem analyzers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFileKind {
    Rust,
    Toml,
    Markdown,
    Json,
    Yaml,
    Lockfile,
    Config,
    OtherText,
}

/// One discovered project file.
#[derive(Debug, Clone)]
pub struct ProjectFile {
    absolute_path: PathBuf,
    relative_path: PathBuf,
    kind: ProjectFileKind,
    byte_length: u64,
    text: Option<String>,
}

impl ProjectFile {
    pub fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub const fn kind(&self) -> ProjectFileKind {
        self.kind
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

/// External evidence a scan frontend can collect for the core analyzers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectTool {
    CargoMetadata,
    CargoCheck,
    Clippy,
    Tests,
    GitStatus,
}

impl ProjectTool {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CargoMetadata => "cargo-metadata",
            Self::CargoCheck => "cargo-check",
            Self::Clippy => "clippy",
            Self::Tests => "tests",
            Self::GitStatus => "git-status",
        }
    }
}

/// Captured stdout/stderr and status from one external project tool.
///
/// The core scanner only consumes this data. It never launches the command.
#[derive(Debug, Clone)]
pub struct ProjectToolOutput {
    pub command: String,
    pub success: bool,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProjectToolOutput {
    pub fn new(
        command: impl Into<String>,
        success: bool,
        status_code: Option<i32>,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            success,
            status_code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }
}

/// Immutable project snapshot consumed by analyzers.
#[derive(Debug, Clone)]
pub struct ProjectContext {
    root: PathBuf,
    profile: ProjectScanProfile,
    files: Vec<ProjectFile>,
    tool_outputs: BTreeMap<ProjectTool, ProjectToolOutput>,
}

impl ProjectContext {
    /// Discovers project files without executing project code or external tools.
    ///
    /// Symlinks are not followed. Generated/vendor directories, diagprint's
    /// `.diagprint` state directory, and `.diagpack` capsule directories are
    /// excluded from discovery.
    pub fn discover(
        root: impl AsRef<Path>,
        profile: ProjectScanProfile,
    ) -> Result<Self, ProjectScanError> {
        let requested_root = root.as_ref();

        let root = fs::canonicalize(requested_root).map_err(|source| ProjectScanError::Io {
            operation: "canonicalize project root",
            path: requested_root.to_path_buf(),
            source,
        })?;

        if !root.is_dir() {
            return Err(ProjectScanError::NotDirectory { path: root });
        }

        let mut files = Vec::new();

        walk_directory(&root, &root, &mut files)?;

        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        Ok(Self {
            root,
            profile,
            files,
            tool_outputs: BTreeMap::new(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub const fn profile(&self) -> ProjectScanProfile {
        self.profile
    }

    pub fn files(&self) -> &[ProjectFile] {
        &self.files
    }

    pub fn file(&self, relative_path: impl AsRef<Path>) -> Option<&ProjectFile> {
        let relative_path = relative_path.as_ref();

        self.files
            .iter()
            .find(|file| file.relative_path == relative_path)
    }

    pub fn has_file(&self, relative_path: impl AsRef<Path>) -> bool {
        self.file(relative_path).is_some()
    }

    pub fn text(&self, relative_path: impl AsRef<Path>) -> Option<&str> {
        self.file(relative_path).and_then(ProjectFile::text)
    }

    pub fn rust_files(&self) -> impl Iterator<Item = &ProjectFile> {
        self.files
            .iter()
            .filter(|file| file.kind == ProjectFileKind::Rust)
    }

    pub fn insert_tool_output(
        &mut self,
        tool: ProjectTool,
        output: ProjectToolOutput,
    ) -> Option<ProjectToolOutput> {
        self.tool_outputs.insert(tool, output)
    }

    pub fn tool_output(&self, tool: ProjectTool) -> Option<&ProjectToolOutput> {
        self.tool_outputs.get(&tool)
    }
}

/// One project-analysis pass.
///
/// Analyzers consume an immutable [`ProjectContext`] and append ordinary
/// structured [`struct@crate::Diagnostic`] values into one shared report.
pub trait ProjectAnalyzer: Send + Sync {
    fn name(&self) -> &'static str;

    fn analyze(&self, context: &ProjectContext, reporter: &Reporter, report: &mut DiagnosticReport);
}

/// Analyzer registry and project-scan coordinator.
pub struct ProjectScanner {
    analyzers: Vec<Box<dyn ProjectAnalyzer>>,
}

impl Default for ProjectScanner {
    fn default() -> Self {
        Self::with_builtin_analyzers()
    }
}

impl ProjectScanner {
    pub const fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    pub fn with_builtin_analyzers() -> Self {
        Self {
            analyzers: analyzers::builtin_analyzers(),
        }
    }

    pub fn register<A>(&mut self, analyzer: A) -> &mut Self
    where
        A: ProjectAnalyzer + 'static,
    {
        self.analyzers.push(Box::new(analyzer));
        self
    }

    pub fn analyzer_count(&self) -> usize {
        self.analyzers.len()
    }

    pub fn scan(&self, context: &ProjectContext, reporter: &Reporter) -> ProjectScan {
        let mut report = DiagnosticReport::new();

        for analyzer in &self.analyzers {
            analyzer.analyze(context, reporter, &mut report);
        }

        ProjectScan {
            root: context.root.clone(),
            profile: context.profile,
            files_scanned: context.files.len(),
            analyzers_run: self.analyzers.len(),
            report,
        }
    }
}

/// Completed project scan plus its unified diagnostic report.
#[derive(Debug, Clone)]
pub struct ProjectScan {
    root: PathBuf,
    profile: ProjectScanProfile,
    files_scanned: usize,
    analyzers_run: usize,
    report: DiagnosticReport,
}

impl ProjectScan {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub const fn profile(&self) -> ProjectScanProfile {
        self.profile
    }

    pub const fn files_scanned(&self) -> usize {
        self.files_scanned
    }

    pub const fn analyzers_run(&self) -> usize {
        self.analyzers_run
    }

    pub fn report(&self) -> &DiagnosticReport {
        &self.report
    }

    pub fn into_report(self) -> DiagnosticReport {
        self.report
    }
}

/// Failure while discovering project contents.
#[derive(Debug)]
pub enum ProjectScanError {
    NotDirectory {
        path: PathBuf,
    },

    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for ProjectScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectory { path } => write!(
                formatter,
                "project scan root is not a directory: {}",
                path.display(),
            ),

            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display(),
            ),
        }
    }
}

impl Error for ProjectScanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::NotDirectory { .. } => None,
        }
    }
}

fn walk_directory(
    root: &Path,
    directory: &Path,
    files: &mut Vec<ProjectFile>,
) -> Result<(), ProjectScanError> {
    let entries = fs::read_dir(directory).map_err(|source| ProjectScanError::Io {
        operation: "read project directory",
        path: directory.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ProjectScanError::Io {
            operation: "read project directory entry",
            path: directory.to_path_buf(),
            source,
        })?;

        let path = entry.path();

        let file_type = entry.file_type().map_err(|source| ProjectScanError::Io {
            operation: "read project file type",
            path: path.clone(),
            source,
        })?;

        // Never traverse symlinks during project discovery.
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            if should_skip_directory(&path) {
                continue;
            }

            walk_directory(root, &path, files)?;

            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let metadata = entry.metadata().map_err(|source| ProjectScanError::Io {
            operation: "read project file metadata",
            path: path.clone(),
            source,
        })?;

        let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();

        let kind = classify_file(&relative_path);

        let text = if metadata.len() <= MAX_TEXT_FILE_BYTES && is_text_candidate(&relative_path) {
            fs::read(&path)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
        } else {
            None
        };

        files.push(ProjectFile {
            absolute_path: path,
            relative_path,
            kind,
            byte_length: metadata.len(),
            text,
        });
    }

    Ok(())
}

fn should_skip_directory(path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str());

    if matches!(
        name,
        Some(".git" | ".diagprint" | "target" | "node_modules" | "vendor")
    ) {
        return true;
    }

    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("diagpack"))
}

fn classify_file(path: &Path) -> ProjectFileKind {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if matches!(file_name, "Cargo.lock" | "flake.lock") {
        return ProjectFileKind::Lockfile;
    }

    if matches!(
        file_name,
        ".gitignore"
            | ".gitattributes"
            | "rust-toolchain"
            | "rust-toolchain.toml"
            | "rustfmt.toml"
            | ".rustfmt.toml"
            | "clippy.toml"
            | ".clippy.toml"
    ) {
        return ProjectFileKind::Config;
    }

    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => ProjectFileKind::Rust,

        Some("toml") => ProjectFileKind::Toml,

        Some("md") | Some("markdown") => ProjectFileKind::Markdown,

        Some("json") => ProjectFileKind::Json,

        Some("yaml") | Some("yml") => ProjectFileKind::Yaml,

        Some("lock") => ProjectFileKind::Lockfile,

        _ => ProjectFileKind::OtherText,
    }
}

fn is_text_candidate(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if file_name.starts_with(".env")
        || matches!(
            file_name,
            ".gitignore"
                | ".gitattributes"
                | "Cargo.lock"
                | "rust-toolchain"
                | "LICENSE"
                | "COPYING"
        )
    {
        return true;
    }

    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "rs" | "toml"
                | "md"
                | "markdown"
                | "txt"
                | "json"
                | "yaml"
                | "yml"
                | "lock"
                | "cfg"
                | "conf"
                | "pem"
                | "key"
        )
    )
}
