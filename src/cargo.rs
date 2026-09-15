//! Cargo build-stream and workspace intelligence.
//!
//! Cargo exposes two complementary machine-readable interfaces:
//!
//! - `cargo metadata --format-version=1` describes packages, targets, and the
//!   resolved dependency graph.
//! - `--message-format=json` describes an individual build as JSON lines.
//!
//! `diagprint` combines those sources without invoking Cargo itself. Callers
//! remain in control of process execution and can feed captured JSON directly
//! into this module.
//!
//! Unknown future Cargo message kinds are preserved rather than rejected.

use crate::{
    CompilerImportError, CompilerImporter, Diagnostic, DocumentationResolver, Reporter, Severity,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    path::PathBuf,
};

#[derive(Debug)]
pub enum CargoImportError {
    Json(serde_json::Error),

    Compiler(CompilerImportError),

    UnsupportedMetadataVersion { version: u64 },

    MissingCompilerDiagnostic,
}

impl fmt::Display for CargoImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => {
                write!(f, "invalid Cargo JSON: {error}")
            }

            Self::Compiler(error) => {
                write!(f, "could not import embedded rustc diagnostic: {error}")
            }

            Self::UnsupportedMetadataVersion { version } => {
                write!(
                    f,
                    "unsupported cargo metadata format version {version}; expected version 1"
                )
            }

            Self::MissingCompilerDiagnostic => {
                write!(
                    f,
                    "Cargo compiler-message did not contain an importable rustc diagnostic"
                )
            }
        }
    }
}

impl Error for CargoImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Compiler(error) => Some(error),

            Self::UnsupportedMetadataVersion { .. } | Self::MissingCompilerDiagnostic => None,
        }
    }
}

impl From<serde_json::Error> for CargoImportError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<CompilerImportError> for CargoImportError {
    fn from(value: CompilerImportError) -> Self {
        Self::Compiler(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CargoTarget {
    pub name: String,

    #[serde(default)]
    pub kind: Vec<String>,

    #[serde(default)]
    pub crate_types: Vec<String>,

    pub src_path: PathBuf,

    #[serde(default)]
    pub edition: String,

    #[serde(default, rename = "required-features")]
    pub required_features: Vec<String>,

    #[serde(default)]
    pub doc: bool,

    #[serde(default)]
    pub doctest: bool,

    #[serde(default)]
    pub test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoPackage {
    pub id: String,
    pub name: String,
    pub version: String,

    pub manifest_path: PathBuf,

    pub source: Option<String>,
    pub edition: String,
    pub rust_version: Option<String>,

    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,

    pub targets: Vec<CargoTarget>,

    pub workspace_member: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoDependencyKind {
    /// `None` means a normal dependency.
    ///
    /// Cargo currently uses values such as `dev` and `build`. Unknown future
    /// values are preserved as strings.
    pub kind: Option<String>,

    /// Optional Cargo target expression such as `cfg(windows)`.
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoDependency {
    /// Dependency name visible to the depending crate. This preserves renamed
    /// dependencies.
    pub name: String,

    pub package_id: String,

    pub package_name: Option<String>,
    pub package_version: Option<String>,

    pub kinds: Vec<CargoDependencyKind>,
}

#[derive(Debug, Clone)]
pub struct CargoWorkspace {
    pub workspace_root: PathBuf,
    pub target_directory: PathBuf,

    pub root_package_id: Option<String>,

    packages: BTreeMap<String, CargoPackage>,

    dependencies: BTreeMap<String, Vec<CargoDependency>>,

    enabled_features: BTreeMap<String, Vec<String>>,
}

impl CargoWorkspace {
    /// Parses output from:
    ///
    /// `cargo metadata --format-version=1`
    pub fn from_metadata_json(input: &str) -> Result<Self, CargoImportError> {
        let raw: RawMetadata = serde_json::from_str(input)?;

        if raw.version != 1 {
            return Err(CargoImportError::UnsupportedMetadataVersion {
                version: raw.version,
            });
        }

        let workspace_members: BTreeSet<String> = raw.workspace_members.into_iter().collect();

        let mut packages = BTreeMap::new();

        for package in raw.packages {
            let id = package.id.clone();
            let workspace_member = workspace_members.contains(&id);

            packages.insert(
                id.clone(),
                CargoPackage {
                    id,
                    name: package.name,
                    version: package.version,
                    manifest_path: package.manifest_path,
                    source: package.source,
                    edition: package.edition,
                    rust_version: package.rust_version,
                    documentation: package.documentation,
                    repository: package.repository,
                    homepage: package.homepage,
                    targets: package.targets,
                    workspace_member,
                },
            );
        }

        let mut dependencies = BTreeMap::new();
        let mut enabled_features = BTreeMap::new();
        let mut root_package_id = None;

        if let Some(resolve) = raw.resolve {
            root_package_id = resolve.root;

            for node in resolve.nodes {
                let node_id = node.id;

                enabled_features.insert(node_id.clone(), node.features);

                let resolved_dependencies = node
                    .deps
                    .into_iter()
                    .map(|dependency| {
                        let package = packages.get(&dependency.pkg);

                        CargoDependency {
                            name: dependency.name,
                            package_id: dependency.pkg,
                            package_name: package.map(|package| package.name.clone()),
                            package_version: package.map(|package| package.version.clone()),
                            kinds: dependency
                                .dep_kinds
                                .into_iter()
                                .map(|kind| CargoDependencyKind {
                                    kind: kind.kind,
                                    target: kind.target,
                                })
                                .collect(),
                        }
                    })
                    .collect();

                dependencies.insert(node_id, resolved_dependencies);
            }
        }

        Ok(Self {
            workspace_root: raw.workspace_root,
            target_directory: raw.target_directory,
            root_package_id,
            packages,
            dependencies,
            enabled_features,
        })
    }

    pub fn package(&self, package_id: &str) -> Option<&CargoPackage> {
        self.packages.get(package_id)
    }

    pub fn root_package(&self) -> Option<&CargoPackage> {
        self.root_package_id
            .as_deref()
            .and_then(|package_id| self.package(package_id))
    }

    pub fn packages(&self) -> impl Iterator<Item = &CargoPackage> {
        self.packages.values()
    }

    pub fn dependencies_of(&self, package_id: &str) -> &[CargoDependency] {
        self.dependencies
            .get(package_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn enabled_features(&self, package_id: &str) -> &[String] {
        self.enabled_features
            .get(package_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Builds a documentation catalog from the exact package versions Cargo
    /// reported in metadata.
    ///
    /// If the resolved graph contains multiple versions of a package,
    /// DocumentationResolver will continue to fail closed for ambiguous
    /// package lookups.
    pub fn documentation_resolver(&self) -> DocumentationResolver {
        let mut resolver = DocumentationResolver::new();

        for package in self.packages.values() {
            resolver.register_package_version(&package.name, &package.version);
        }

        resolver
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoPackageIdentity {
    pub id: String,

    pub name: Option<String>,
    pub version: Option<String>,

    pub workspace_member: bool,
}

impl CargoPackageIdentity {
    pub fn display_name(&self) -> String {
        match (&self.name, &self.version) {
            (Some(name), Some(version)) => {
                format!("{name} {version}")
            }

            (Some(name), None) => name.clone(),

            _ => self.id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CargoCompilerDiagnostic {
    pub package: CargoPackageIdentity,

    pub manifest_path: PathBuf,

    pub target: CargoTarget,

    pub dependencies: Vec<CargoDependency>,

    pub diagnostic: Diagnostic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CargoProfile {
    pub opt_level: String,

    /// Cargo allows integer, string, or null debug-info representations.
    pub debuginfo: Value,

    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub test: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CargoArtifact {
    pub package: CargoPackageIdentity,

    pub manifest_path: PathBuf,

    pub target: CargoTarget,

    pub profile: CargoProfile,

    pub features: Vec<String>,

    pub filenames: Vec<PathBuf>,

    pub executable: Option<PathBuf>,

    /// `true` means the existing artifacts were already up-to-date and rustc
    /// was not executed for this step.
    pub fresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoBuildScript {
    pub package: CargoPackageIdentity,

    pub linked_libs: Vec<String>,

    pub linked_paths: Vec<String>,

    pub cfgs: Vec<String>,

    pub env: Vec<(String, String)>,

    pub out_dir: PathBuf,
}

impl CargoBuildScript {
    pub fn links_native_code(&self) -> bool {
        !self.linked_libs.is_empty() || !self.linked_paths.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CargoBuildFinished {
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CargoUnknownMessage {
    pub reason: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone)]
pub enum CargoMessage {
    CompilerDiagnostic(Box<CargoCompilerDiagnostic>),

    Artifact(Box<CargoArtifact>),

    BuildScript(Box<CargoBuildScript>),

    BuildFinished(CargoBuildFinished),

    /// Forward-compatible preservation of Cargo message kinds which this
    /// version of diagprint does not understand yet.
    Unknown(Box<CargoUnknownMessage>),
}

impl CargoMessage {
    pub fn reason(&self) -> &str {
        match self {
            Self::CompilerDiagnostic(_) => "compiler-message",
            Self::Artifact(_) => "compiler-artifact",
            Self::BuildScript(_) => "build-script-executed",
            Self::BuildFinished(_) => "build-finished",

            Self::Unknown(message) => message.reason.as_deref().unwrap_or("unknown"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CargoStreamImporter {
    compiler: CompilerImporter,

    workspace: Option<CargoWorkspace>,
}

impl CargoStreamImporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compiler_importer(mut self, compiler: CompilerImporter) -> Self {
        self.compiler = compiler;
        self
    }

    pub fn workspace(mut self, workspace: CargoWorkspace) -> Self {
        self.workspace = Some(workspace);
        self
    }

    pub fn workspace_ref(&self) -> Option<&CargoWorkspace> {
        self.workspace.as_ref()
    }

    /// Parses one line emitted by Cargo with `--message-format=json`.
    ///
    /// Non-JSON lines are ignored because Cargo cannot force arbitrary tools,
    /// procedural macros, or executed programs to emit JSON.
    pub fn import_line(
        &self,
        reporter: &Reporter,
        line: &str,
    ) -> Result<Option<CargoMessage>, CargoImportError> {
        let line = line.trim();

        if line.is_empty() || !line.starts_with('{') {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(line)?;

        let reason = value.get("reason").and_then(Value::as_str);

        match reason {
            Some("compiler-message") => self.import_compiler_message(reporter, value).map(Some),

            Some("compiler-artifact") => self.import_artifact(value).map(Some),

            Some("build-script-executed") => self.import_build_script(value).map(Some),

            Some("build-finished") => {
                let raw: RawBuildFinished = serde_json::from_value(value)?;

                Ok(Some(CargoMessage::BuildFinished(CargoBuildFinished {
                    success: raw.success,
                })))
            }

            _ => Ok(Some(CargoMessage::Unknown(Box::new(CargoUnknownMessage {
                reason: reason.map(ToOwned::to_owned),
                raw: value,
            })))),
        }
    }

    fn import_compiler_message(
        &self,
        reporter: &Reporter,
        value: Value,
    ) -> Result<CargoMessage, CargoImportError> {
        let raw: RawCompilerMessage = serde_json::from_value(value)?;

        let rustc_json = serde_json::to_string(&raw.message)?;

        let Some(mut diagnostic) = self.compiler.import_line(reporter, &rustc_json)? else {
            return Err(CargoImportError::MissingCompilerDiagnostic);
        };

        let package = self.package_identity(&raw.package_id);

        diagnostic = diagnostic.note(format!("cargo package: {}", package.display_name()));

        diagnostic = diagnostic.note(format!("cargo package id: {}", raw.package_id));

        diagnostic = diagnostic.note(format!("cargo manifest: {}", raw.manifest_path.display()));

        let kinds = if raw.target.kind.is_empty() {
            "unknown".to_owned()
        } else {
            raw.target.kind.join(", ")
        };

        diagnostic = diagnostic.note(format!(
            "cargo target: {} [{kinds}] edition {}",
            raw.target.name, raw.target.edition
        ));

        diagnostic = diagnostic.note(format!(
            "cargo target source: {}",
            raw.target.src_path.display()
        ));

        if !raw.target.required_features.is_empty() {
            diagnostic = diagnostic.note(format!(
                "cargo target required features: {}",
                raw.target.required_features.join(", ")
            ));
        }

        let dependencies = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.dependencies_of(&raw.package_id).to_vec())
            .unwrap_or_default();

        if !dependencies.is_empty() {
            diagnostic = diagnostic.note(format!(
                "cargo resolved direct dependencies: {}",
                dependencies.len()
            ));
        }

        Ok(CargoMessage::CompilerDiagnostic(Box::new(
            CargoCompilerDiagnostic {
                package,
                manifest_path: raw.manifest_path,
                target: raw.target,
                dependencies,
                diagnostic,
            },
        )))
    }

    fn import_artifact(&self, value: Value) -> Result<CargoMessage, CargoImportError> {
        let raw: RawArtifact = serde_json::from_value(value)?;

        Ok(CargoMessage::Artifact(Box::new(CargoArtifact {
            package: self.package_identity(&raw.package_id),
            manifest_path: raw.manifest_path,
            target: raw.target,
            profile: raw.profile,
            features: raw.features,
            filenames: raw.filenames,
            executable: raw.executable,
            fresh: raw.fresh,
        })))
    }

    fn import_build_script(&self, value: Value) -> Result<CargoMessage, CargoImportError> {
        let raw: RawBuildScript = serde_json::from_value(value)?;

        Ok(CargoMessage::BuildScript(Box::new(CargoBuildScript {
            package: self.package_identity(&raw.package_id),
            linked_libs: raw.linked_libs,
            linked_paths: raw.linked_paths,
            cfgs: raw.cfgs,
            env: raw.env,
            out_dir: raw.out_dir,
        })))
    }

    fn package_identity(&self, package_id: &str) -> CargoPackageIdentity {
        let package = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.package(package_id));

        CargoPackageIdentity {
            id: package_id.to_owned(),
            name: package.map(|package| package.name.clone()),
            version: package.map(|package| package.version.clone()),
            workspace_member: package.is_some_and(|package| package.workspace_member),
        }
    }
}

/// Aggregate intelligence for one Cargo build stream.
///
/// This type never executes Cargo. Feed each parsed [`CargoMessage`] to
/// [`CargoBuildSummary::observe`].
#[derive(Debug, Clone, Default)]
pub struct CargoBuildSummary {
    pub compiler_diagnostics: usize,
    pub errors: usize,
    pub warnings: usize,

    pub artifacts: usize,
    pub fresh_artifacts: usize,

    pub build_scripts: usize,
    pub native_link_build_scripts: usize,

    pub unknown_messages: usize,

    pub build_success: Option<bool>,

    packages: BTreeSet<String>,
}

impl CargoBuildSummary {
    pub fn observe(&mut self, message: &CargoMessage) {
        match message {
            CargoMessage::CompilerDiagnostic(message) => {
                self.compiler_diagnostics += 1;
                self.packages.insert(message.package.id.clone());

                match message.diagnostic.severity {
                    Severity::Fatal | Severity::Error => {
                        self.errors += 1;
                    }

                    Severity::Warning => {
                        self.warnings += 1;
                    }

                    Severity::Trace | Severity::Debug | Severity::Info => {}
                }
            }

            CargoMessage::Artifact(artifact) => {
                self.artifacts += 1;
                self.packages.insert(artifact.package.id.clone());

                if artifact.fresh {
                    self.fresh_artifacts += 1;
                }
            }

            CargoMessage::BuildScript(script) => {
                self.build_scripts += 1;
                self.packages.insert(script.package.id.clone());

                if script.links_native_code() {
                    self.native_link_build_scripts += 1;
                }
            }

            CargoMessage::BuildFinished(finished) => {
                self.build_success = Some(finished.success);
            }

            CargoMessage::Unknown(_) => {
                self.unknown_messages += 1;
            }
        }
    }

    pub fn package_ids(&self) -> impl Iterator<Item = &str> {
        self.packages.iter().map(String::as_str)
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    pub fn successful(&self) -> Option<bool> {
        self.build_success
    }
}

#[derive(Debug, Deserialize)]
struct RawMetadata {
    #[serde(default)]
    packages: Vec<RawPackage>,

    #[serde(default)]
    workspace_members: Vec<String>,

    #[serde(default)]
    resolve: Option<RawResolve>,

    target_directory: PathBuf,

    workspace_root: PathBuf,

    version: u64,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    id: String,
    name: String,
    version: String,

    manifest_path: PathBuf,

    #[serde(default)]
    source: Option<String>,

    #[serde(default)]
    edition: String,

    #[serde(default)]
    rust_version: Option<String>,

    #[serde(default)]
    documentation: Option<String>,

    #[serde(default)]
    repository: Option<String>,

    #[serde(default)]
    homepage: Option<String>,

    #[serde(default)]
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct RawResolve {
    #[serde(default)]
    nodes: Vec<RawResolveNode>,

    #[serde(default)]
    root: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawResolveNode {
    id: String,

    #[serde(default)]
    deps: Vec<RawResolvedDependency>,

    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawResolvedDependency {
    name: String,

    pkg: String,

    #[serde(default)]
    dep_kinds: Vec<RawDependencyKind>,
}

#[derive(Debug, Deserialize)]
struct RawDependencyKind {
    #[serde(default)]
    kind: Option<String>,

    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCompilerMessage {
    package_id: String,

    manifest_path: PathBuf,

    target: CargoTarget,

    message: Value,
}

#[derive(Debug, Deserialize)]
struct RawArtifact {
    package_id: String,

    manifest_path: PathBuf,

    target: CargoTarget,

    profile: CargoProfile,

    #[serde(default)]
    features: Vec<String>,

    #[serde(default)]
    filenames: Vec<PathBuf>,

    #[serde(default)]
    executable: Option<PathBuf>,

    #[serde(default)]
    fresh: bool,
}

impl<'de> Deserialize<'de> for CargoProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawProfile {
            #[serde(default)]
            opt_level: String,

            #[serde(default)]
            debuginfo: Value,

            #[serde(default)]
            debug_assertions: bool,

            #[serde(default)]
            overflow_checks: bool,

            #[serde(default)]
            test: bool,
        }

        let raw = RawProfile::deserialize(deserializer)?;

        Ok(Self {
            opt_level: raw.opt_level,
            debuginfo: raw.debuginfo,
            debug_assertions: raw.debug_assertions,
            overflow_checks: raw.overflow_checks,
            test: raw.test,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawBuildScript {
    package_id: String,

    #[serde(default)]
    linked_libs: Vec<String>,

    #[serde(default)]
    linked_paths: Vec<String>,

    #[serde(default)]
    cfgs: Vec<String>,

    #[serde(default)]
    env: Vec<(String, String)>,

    out_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawBuildFinished {
    success: bool,
}
