use super::{ProjectAnalyzer, ProjectContext, ProjectFile, ProjectFileKind, ProjectTool};
use crate::{
    Applicability, CargoMessage, CargoStreamImporter, CargoWorkspace, Diagnostic, DiagnosticReport,
    Reporter, Severity, Suggestion,
};
use std::path::Path;

pub(super) fn builtin_analyzers() -> Vec<Box<dyn ProjectAnalyzer>> {
    vec![
        Box::new(WorkspaceAnalyzer),
        Box::new(ManifestAnalyzer),
        Box::new(SourceInventoryAnalyzer),
        Box::new(UnsafeInventoryAnalyzer),
        Box::new(DependencyAnalyzer),
        Box::new(ConfigurationAnalyzer),
        Box::new(DocumentationAnalyzer),
        Box::new(LicenseAnalyzer),
        Box::new(SecurityAnalyzer),
        Box::new(GitAnalyzer),
        Box::new(CargoAnalyzer),
        Box::new(CompilerAnalyzer),
        Box::new(ClippyAnalyzer),
        Box::new(TestAnalyzer),
        Box::new(RemediationAnalyzer),
    ]
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WorkspaceAnalyzer;

impl ProjectAnalyzer for WorkspaceAnalyzer {
    fn name(&self) -> &'static str {
        "workspace"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let Some(manifest) = manifest_value(context) else {
            return;
        };

        let Some(workspace) = manifest.get("workspace").and_then(toml::Value::as_table) else {
            return;
        };

        let member_count = workspace
            .get("members")
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len);

        let exclude_count = workspace
            .get("exclude")
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len);

        let diagnostic = reporter
            .info(format!(
                "Cargo workspace declares {member_count} member pattern(s)"
            ))
            .code("project::workspace::inventory")
            .label("Cargo.toml", 1, None, None, Some("workspace manifest"))
            .note(format!("workspace exclusions: {exclude_count}"));

        push_unique(report, diagnostic);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ManifestAnalyzer;

impl ProjectAnalyzer for ManifestAnalyzer {
    fn name(&self) -> &'static str {
        "manifest"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let Some(text) = context.text("Cargo.toml") else {
            push_unique(
                report,
                reporter
                    .warning("project root does not contain Cargo.toml")
                    .code("project::manifest::missing"),
            );

            return;
        };

        let manifest = match toml::from_str::<toml::Value>(text) {
            Ok(manifest) => manifest,

            Err(error) => {
                push_unique(
                    report,
                    reporter
                        .error(format!("Cargo.toml could not be parsed: {error}"))
                        .code("project::manifest::invalid")
                        .label("Cargo.toml", 1, None, None, Some("manifest parse failure")),
                );

                return;
            }
        };

        if let Some(package) = manifest.get("package").and_then(toml::Value::as_table) {
            let name = package
                .get("name")
                .and_then(toml::Value::as_str)
                .unwrap_or("<unnamed>");

            let version = package
                .get("version")
                .and_then(toml::Value::as_str)
                .unwrap_or("<unspecified>");

            let edition = package
                .get("edition")
                .and_then(toml::Value::as_str)
                .unwrap_or("<unspecified>");

            let mut diagnostic = reporter
                .info(format!("Cargo package {name} {version}"))
                .code("project::manifest::package")
                .label("Cargo.toml", 1, None, None, Some("package manifest"))
                .note(format!("Rust edition: {edition}"));

            if let Some(rust_version) = package.get("rust-version").and_then(toml::Value::as_str) {
                diagnostic = diagnostic.note(format!("declared MSRV: {rust_version}"));
            } else {
                diagnostic = diagnostic.note("rust-version/MSRV is not declared");
            }

            push_unique(report, diagnostic);
        } else if manifest.get("workspace").is_some() {
            push_unique(
                report,
                reporter
                    .info("Cargo.toml is a virtual workspace manifest")
                    .code("project::manifest::virtual-workspace")
                    .label("Cargo.toml", 1, None, None, Some("workspace manifest")),
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SourceInventoryAnalyzer;

impl ProjectAnalyzer for SourceInventoryAnalyzer {
    fn name(&self) -> &'static str {
        "source-inventory"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let rust_files = context
            .files()
            .iter()
            .filter(|file| file.kind() == ProjectFileKind::Rust)
            .count();

        let toml_files = context
            .files()
            .iter()
            .filter(|file| file.kind() == ProjectFileKind::Toml)
            .count();

        let total_rust_lines = context
            .rust_files()
            .filter_map(ProjectFile::text)
            .map(str::lines)
            .map(Iterator::count)
            .sum::<usize>();

        push_unique(
            report,
            reporter
                .info(format!(
                    "discovered {} project file(s)",
                    context.files().len(),
                ))
                .code("project::source::inventory")
                .note(format!("Rust files: {rust_files}"))
                .note(format!("Rust source lines: {total_rust_lines}"))
                .note(format!("TOML files: {toml_files}")),
        );

        for file in context.rust_files() {
            let Some(text) = file.text() else {
                continue;
            };

            let line_count = text.lines().count();

            if line_count < 3_000 {
                continue;
            }

            push_unique(
                report,
                reporter
                    .info(format!(
                        "large Rust source file: {} ({line_count} lines)",
                        file.relative_path().display(),
                    ))
                    .code("project::source::large-file")
                    .label(
                        path_string(file.relative_path()),
                        1,
                        None,
                        None,
                        Some("large source file"),
                    ),
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnsafeInventoryAnalyzer;

impl ProjectAnalyzer for UnsafeInventoryAnalyzer {
    fn name(&self) -> &'static str {
        "unsafe-inventory"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        for file in context.rust_files() {
            let Some(text) = file.text() else {
                continue;
            };

            let matches = text
                .lines()
                .enumerate()
                .filter(|(_, line)| contains_unsafe_token(line))
                .map(|(index, _)| index + 1)
                .collect::<Vec<_>>();

            if matches.is_empty() {
                continue;
            }

            let first_line = u32::try_from(matches[0]).unwrap_or(u32::MAX);

            push_unique(
                report,
                reporter
                    .info(format!(
                        "{} textual unsafe marker(s) in {}",
                        matches.len(),
                        file.relative_path().display(),
                    ))
                    .code(
                        "project::unsafe::inventory",
                    )
                    .label(
                        path_string(file.relative_path()),
                        first_line,
                        None,
                        None,
                        Some(
                            "first textual unsafe marker",
                        ),
                    )
                    .note(
                        "This is a textual unsafe inventory, not a semantic proof of unsafe behavior.",
                    )
                    .help(
                        "Review unsafe blocks and declarations for documented invariants and minimal scope.",
                    ),
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DependencyAnalyzer;

impl ProjectAnalyzer for DependencyAnalyzer {
    fn name(&self) -> &'static str {
        "dependencies"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let Some(manifest) = manifest_value(context) else {
            return;
        };

        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(table) = manifest.get(section).and_then(toml::Value::as_table) {
                inspect_dependency_table(section, table, reporter, report);
            }
        }

        if let Some(table) = manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("dependencies"))
            .and_then(toml::Value::as_table)
        {
            inspect_dependency_table("workspace.dependencies", table, reporter, report);
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ConfigurationAnalyzer;

impl ProjectAnalyzer for ConfigurationAnalyzer {
    fn name(&self) -> &'static str {
        "configuration"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let known = [
            "rust-toolchain.toml",
            "rust-toolchain",
            "rustfmt.toml",
            ".rustfmt.toml",
            "clippy.toml",
            ".clippy.toml",
            "deny.toml",
            ".cargo/config.toml",
            ".cargo/config",
        ];

        let present = known
            .iter()
            .copied()
            .filter(|path| context.has_file(path))
            .collect::<Vec<_>>();

        if !present.is_empty() {
            push_unique(
                report,
                reporter
                    .info(format!(
                        "{} recognized project configuration file(s)",
                        present.len(),
                    ))
                    .code("project::configuration::inventory")
                    .note(present.join(", ")),
            );
        }

        if context.has_file(".cargo/config") && context.has_file(".cargo/config.toml") {
            push_unique(
                report,
                reporter
                    .warning(
                        "both .cargo/config and .cargo/config.toml are present",
                    )
                    .code(
                        "project::configuration::cargo-config-overlap",
                    )
                    .help(
                        "Keep one canonical Cargo configuration file to avoid ambiguous maintenance.",
                    ),
            );
        }

        if context.has_file("rustfmt.toml") && context.has_file(".rustfmt.toml") {
            push_unique(
                report,
                reporter
                    .warning("both rustfmt.toml and .rustfmt.toml are present")
                    .code("project::configuration::rustfmt-overlap"),
            );
        }

        if context.has_file("clippy.toml") && context.has_file(".clippy.toml") {
            push_unique(
                report,
                reporter
                    .warning("both clippy.toml and .clippy.toml are present")
                    .code("project::configuration::clippy-overlap"),
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DocumentationAnalyzer;

impl ProjectAnalyzer for DocumentationAnalyzer {
    fn name(&self) -> &'static str {
        "documentation"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let has_readme = context
            .files()
            .iter()
            .filter(|file| file.relative_path().components().count() == 1)
            .filter_map(|file| {
                file.relative_path()
                    .file_name()
                    .and_then(|name| name.to_str())
            })
            .any(|name| name.to_ascii_lowercase().starts_with("readme"));

        if !has_readme {
            push_unique(
                report,
                reporter
                    .info("project root does not contain a README")
                    .code("project::documentation::readme-missing"),
            );
        }

        if let Some(lib) = context.text("src/lib.rs") {
            let has_crate_docs = lib
                .lines()
                .take(40)
                .map(str::trim_start)
                .any(|line| line.starts_with("//!"));

            if !has_crate_docs {
                push_unique(
                    report,
                    reporter
                        .info(
                            "library crate has no crate-level //! documentation near the top of src/lib.rs",
                        )
                        .code(
                            "project::documentation::crate-docs",
                        )
                        .label(
                            "src/lib.rs",
                            1,
                            None,
                            None,
                            Some("crate root"),
                        ),
                );
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LicenseAnalyzer;

impl ProjectAnalyzer for LicenseAnalyzer {
    fn name(&self) -> &'static str {
        "license"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let manifest = manifest_value(context);

        let declared = manifest
            .as_ref()
            .and_then(package_or_workspace_package)
            .is_some_and(|package| {
                package.get("license").is_some() || package.get("license-file").is_some()
            });

        let license_file = context
            .files()
            .iter()
            .filter(|file| file.relative_path().components().count() == 1)
            .filter_map(|file| {
                file.relative_path()
                    .file_name()
                    .and_then(|name| name.to_str())
            })
            .any(|name| {
                let name = name.to_ascii_lowercase();

                name.starts_with("license") || name.starts_with("copying")
            });

        if !declared && !license_file {
            push_unique(
                report,
                reporter
                    .warning(
                        "no package license declaration or root license file was detected",
                    )
                    .code(
                        "project::license::missing",
                    )
                    .help(
                        "Declare package.license/package.license-file and include the applicable license text.",
                    ),
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SecurityAnalyzer;

impl ProjectAnalyzer for SecurityAnalyzer {
    fn name(&self) -> &'static str {
        "security"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        for file in context.files() {
            let file_name = file
                .relative_path()
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();

            if sensitive_filename(file_name) {
                push_unique(
                    report,
                    reporter
                        .warning(format!(
                            "sensitive-looking file is present in the project tree: {}",
                            file.relative_path().display(),
                        ))
                        .code(
                            "project::security::sensitive-file",
                        )
                        .label(
                            path_string(
                                file.relative_path(),
                            ),
                            1,
                            None,
                            None,
                            Some(
                                "review handling of this file",
                            ),
                        )
                        .suggestion(
                            Suggestion::new(
                                "Review sensitive file handling",
                            )
                            .explanation(
                                "Confirm that credentials, environment secrets, or local-only configuration are not committed or distributed unintentionally.",
                            )
                            .applicability(
                                Applicability::Manual,
                            ),
                        ),
                );
            }

            let Some(text) = file.text() else {
                continue;
            };

            let private_key_line = text
                .lines()
                .enumerate()
                .find(|(_, line)| {
                    line.contains("-----BEGIN PRIVATE KEY-----")
                        || line.contains("-----BEGIN RSA PRIVATE KEY-----")
                        || line.contains("-----BEGIN OPENSSH PRIVATE KEY-----")
                })
                .map(|(index, _)| index + 1);

            let Some(line) = private_key_line else {
                continue;
            };

            push_unique(
                report,
                reporter
                    .warning(format!(
                        "possible private-key material detected in {}",
                        file.relative_path().display(),
                    ))
                    .code(
                        "project::security::private-key-material",
                    )
                    .label(
                        path_string(file.relative_path()),
                        u32::try_from(line)
                            .unwrap_or(u32::MAX),
                        None,
                        None,
                        Some(
                            "private-key header detected",
                        ),
                    )
                    .help(
                        "Verify that this key material is intentional test data and is not a live credential.",
                    ),
            );
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GitAnalyzer;

impl ProjectAnalyzer for GitAnalyzer {
    fn name(&self) -> &'static str {
        "git"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let Some(output) = context.tool_output(ProjectTool::GitStatus) else {
            return;
        };

        if !output.success {
            push_unique(
                report,
                reporter
                    .warning("Git status collection failed")
                    .code("project::git::status-failed")
                    .note(tool_status_note(output)),
            );

            return;
        }

        let mut branch = None;
        let mut changes = 0usize;

        for line in output.stdout.lines() {
            if line.starts_with("## ") {
                branch = Some(line.to_owned());
            } else if !line.trim().is_empty() {
                changes += 1;
            }
        }

        let mut diagnostic = if changes == 0 {
            reporter
                .info("Git working tree is clean")
                .code("project::git::clean")
        } else {
            reporter
                .warning(format!(
                    "Git working tree contains {changes} changed/untracked path(s)"
                ))
                .code("project::git::dirty")
        };

        if let Some(branch) = branch {
            diagnostic = diagnostic.note(branch);
        }

        push_unique(report, diagnostic);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CargoAnalyzer;

impl ProjectAnalyzer for CargoAnalyzer {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let Some(output) = context.tool_output(ProjectTool::CargoMetadata) else {
            return;
        };

        let workspace = match CargoWorkspace::from_metadata_json(&output.stdout) {
            Ok(workspace) => workspace,

            Err(error) => {
                push_unique(
                    report,
                    reporter
                        .warning(format!("Cargo metadata could not be imported: {error}"))
                        .code("project::cargo::metadata-invalid")
                        .note(tool_status_note(output)),
                );

                return;
            }
        };

        let package_count = workspace.packages().count();

        let member_count = workspace
            .packages()
            .filter(|package| package.workspace_member)
            .count();

        let dependency_edges = workspace
            .packages()
            .map(|package| workspace.dependencies_of(&package.id).len())
            .sum::<usize>();

        push_unique(
            report,
            reporter
                .info(format!("Cargo resolved {package_count} package(s)"))
                .code("project::cargo::metadata")
                .note(format!("workspace packages: {member_count}"))
                .note(format!(
                    "resolved direct dependency edges: {dependency_edges}"
                )),
        );
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CompilerAnalyzer;

impl ProjectAnalyzer for CompilerAnalyzer {
    fn name(&self) -> &'static str {
        "compiler"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        import_cargo_diagnostics(
            context,
            reporter,
            report,
            ProjectTool::CargoCheck,
            "cargo-check",
            "project::compiler",
        );
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ClippyAnalyzer;

impl ProjectAnalyzer for ClippyAnalyzer {
    fn name(&self) -> &'static str {
        "clippy"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        import_cargo_diagnostics(
            context,
            reporter,
            report,
            ProjectTool::Clippy,
            "clippy",
            "project::clippy",
        );
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TestAnalyzer;

impl ProjectAnalyzer for TestAnalyzer {
    fn name(&self) -> &'static str {
        "tests"
    }

    fn analyze(
        &self,
        context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let Some(output) = context.tool_output(ProjectTool::Tests) else {
            return;
        };

        let diagnostic = if output.success {
            reporter
                .info("project test command completed successfully")
                .code("project::tests::passed")
                .note(output.command.clone())
        } else {
            reporter
                .error("project test command failed")
                .code("project::tests::failed")
                .note(output.command.clone())
                .note(tool_status_note(output))
                .help("Run the test command directly for full test-harness output.")
        };

        push_unique(report, diagnostic);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RemediationAnalyzer;

impl ProjectAnalyzer for RemediationAnalyzer {
    fn name(&self) -> &'static str {
        "remediation"
    }

    fn analyze(
        &self,
        _context: &ProjectContext,
        reporter: &Reporter,
        report: &mut DiagnosticReport,
    ) {
        let suggestions = report
            .iter()
            .map(|diagnostic| diagnostic.suggestions.len())
            .sum::<usize>();

        let machine_applicable = report
            .iter()
            .flat_map(|diagnostic| diagnostic.suggestions.iter())
            .filter(|suggestion| suggestion.applicability.can_apply_automatically())
            .count();

        if suggestions == 0 {
            return;
        }

        push_unique(
            report,
            reporter
                .info(format!(
                    "{suggestions} remediation suggestion(s) are attached to scan findings"
                ))
                .code("project::remediation::inventory")
                .note(format!(
                    "machine-applicable suggestions: {machine_applicable}"
                ))
                .note(format!(
                    "manual or guarded suggestions: {}",
                    suggestions.saturating_sub(machine_applicable,),
                )),
        );
    }
}

fn inspect_dependency_table(
    section: &str,
    table: &toml::map::Map<String, toml::Value>,
    reporter: &Reporter,
    report: &mut DiagnosticReport,
) {
    for (name, value) in table {
        if value.as_str() == Some("*") {
            dependency_warning(
                reporter,
                report,
                name,
                section,
                "uses a wildcard version requirement",
                "project::dependency::wildcard",
            );

            continue;
        }

        let Some(config) = value.as_table() else {
            continue;
        };

        if config.get("version").and_then(toml::Value::as_str) == Some("*") {
            dependency_warning(
                reporter,
                report,
                name,
                section,
                "uses a wildcard version requirement",
                "project::dependency::wildcard",
            );
        }

        if config.get("git").is_some() && config.get("rev").is_none() && config.get("tag").is_none()
        {
            dependency_warning(
                reporter,
                report,
                name,
                section,
                "uses a floating Git dependency without rev or tag pinning",
                "project::dependency::floating-git",
            );
        }

        if let Some(path) = config.get("path").and_then(toml::Value::as_str) {
            if Path::new(path)
                .components()
                .next()
                .is_some_and(|component| matches!(component, std::path::Component::ParentDir))
            {
                push_unique(
                    report,
                    reporter
                        .info(format!(
                            "dependency {section}.{name} points outside the project root"
                        ))
                        .code("project::dependency::external-path")
                        .label("Cargo.toml", 1, None, None, Some("dependency declaration"))
                        .note(format!("dependency path: {path}")),
                );
            }
        }
    }
}

fn dependency_warning(
    reporter: &Reporter,
    report: &mut DiagnosticReport,
    name: &str,
    section: &str,
    message: &str,
    code: &str,
) {
    push_unique(
        report,
        reporter
            .warning(format!(
                "dependency {section}.{name} {message}"
            ))
            .code(code)
            .label(
                "Cargo.toml",
                1,
                None,
                None,
                Some("dependency declaration"),
            )
            .suggestion(
                Suggestion::new(
                    "Pin the dependency more precisely",
                )
                .explanation(
                    "Use an explicit compatible version, immutable revision, or reviewed tag where reproducibility requires it.",
                )
                .applicability(
                    Applicability::Manual,
                ),
            ),
    );
}

fn import_cargo_diagnostics(
    context: &ProjectContext,
    reporter: &Reporter,
    report: &mut DiagnosticReport,
    tool: ProjectTool,
    analyzer_name: &str,
    summary_code: &str,
) {
    let Some(output) = context.tool_output(tool) else {
        return;
    };

    let workspace = metadata_workspace(context);

    let mut importer = CargoStreamImporter::new();

    if let Some(workspace) = workspace {
        importer = importer.workspace(workspace);
    }

    let mut imported = 0usize;
    let mut imported_errors = 0usize;
    let mut import_failures = 0usize;

    for line in output.stdout.lines() {
        match importer.import_line(reporter, line) {
            Ok(Some(CargoMessage::CompilerDiagnostic(message))) => {
                let message = *message;

                let diagnostic = message
                    .diagnostic
                    .note(format!("project analyzer: {analyzer_name}"));

                if diagnostic.severity >= Severity::Error {
                    imported_errors += 1;
                }

                if push_unique(report, diagnostic) {
                    imported += 1;
                }
            }

            Ok(_) => {}

            Err(_) => {
                import_failures += 1;
            }
        }
    }

    if import_failures > 0 {
        push_unique(
            report,
            reporter
                .warning(format!(
                    "{analyzer_name} produced {import_failures} structured message(s) that could not be imported"
                ))
                .code(format!(
                    "{summary_code}::import-warning"
                )),
        );
    }

    if output.success {
        push_unique(
            report,
            reporter
                .info(format!(
                    "{analyzer_name} completed; {imported} unique compiler diagnostic(s) imported"
                ))
                .code(format!("{summary_code}::completed")),
        );
    } else if imported_errors == 0 {
        push_unique(
            report,
            reporter
                .error(format!(
                    "{analyzer_name} failed without an imported error diagnostic"
                ))
                .code(format!("{summary_code}::command-failed"))
                .note(output.command.clone())
                .note(tool_status_note(output)),
        );
    }
}

fn metadata_workspace(context: &ProjectContext) -> Option<CargoWorkspace> {
    context
        .tool_output(ProjectTool::CargoMetadata)
        .and_then(|output| CargoWorkspace::from_metadata_json(&output.stdout).ok())
}

fn manifest_value(context: &ProjectContext) -> Option<toml::Value> {
    context
        .text("Cargo.toml")
        .and_then(|text| toml::from_str::<toml::Value>(text).ok())
}

fn package_or_workspace_package(
    manifest: &toml::Value,
) -> Option<&toml::map::Map<String, toml::Value>> {
    manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .or_else(|| {
            manifest
                .get("workspace")
                .and_then(|workspace| workspace.get("package"))
                .and_then(toml::Value::as_table)
        })
}

fn contains_unsafe_token(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or_default();

    code.split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|token| token == "unsafe")
}

fn sensitive_filename(file_name: &str) -> bool {
    let name = file_name.to_ascii_lowercase();

    if matches!(
        name.as_str(),
        ".env.example" | ".env.sample" | ".env.template"
    ) {
        return false;
    }

    name == ".env"
        || name.starts_with(".env.")
        || name == "credentials"
        || name == "credentials.json"
        || name == "secrets.toml"
        || name == "secrets.json"
        || name == "id_rsa"
        || name == "id_ed25519"
}

fn tool_status_note(output: &super::ProjectToolOutput) -> String {
    match output.status_code {
        Some(code) => format!("command exit status: {code}"),

        None => "command did not produce an exit status".to_owned(),
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn push_unique(report: &mut DiagnosticReport, diagnostic: Diagnostic) -> bool {
    let fingerprint = diagnostic.fingerprint();

    if report
        .iter()
        .any(|existing| existing.fingerprint() == fingerprint)
    {
        return false;
    }

    report.push(diagnostic);

    true
}
