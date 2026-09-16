use diagprint::{
    Reporter,
    project_scan::{ProjectContext, ProjectScanProfile, ProjectScanner},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("project-scan-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

fn temporary_project(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-project-scan-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent directory should be created");
    }

    fs::write(path, contents).expect("test file should be written");
}

#[test]
fn project_context_discovers_files_but_skips_generated_and_diagprint_directories() {
    let root = temporary_project("discovery");

    write(
        &root,
        "Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    );

    write(&root, "src/lib.rs", "pub fn hello() {}\n");

    write(
        &root,
        "target/generated.rs",
        "compile_error!(\"should not be scanned\");\n",
    );

    write(&root, "vendor/dependency.rs", "unsafe fn external() {}\n");

    write(
        &root,
        ".diagprint/history/run-000000.json",
        "{\"should\":\"not be scanned\"}\n",
    );

    write(
        &root,
        "reports/old-scan.diagpack/manifest.json",
        "{\"should\":\"not be scanned\"}\n",
    );

    let context = ProjectContext::discover(&root, ProjectScanProfile::Static)
        .expect("project context should be discovered");

    assert!(context.has_file("Cargo.toml"));
    assert!(context.has_file("src/lib.rs"));

    assert!(!context.has_file("target/generated.rs"));
    assert!(!context.has_file("vendor/dependency.rs"));

    assert!(!context.has_file(".diagprint/history/run-000000.json"));

    assert!(!context.has_file("reports/old-scan.diagpack/manifest.json"));

    fs::remove_dir_all(root).expect("test project should clean up");
}

#[test]
fn builtin_scanner_finds_manifest_dependency_unsafe_license_and_security_findings() {
    let root = temporary_project("findings");

    write(
        &root,
        "Cargo.toml",
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
floating = "*"
"#,
    );

    write(
        &root,
        "src/lib.rs",
        r#"pub unsafe fn raw_access() {
    // test only
}
"#,
    );

    write(&root, ".env", "EXAMPLE_ONLY=true\n");

    write(&root, "README.md", "# Demo\n");

    let context = ProjectContext::discover(&root, ProjectScanProfile::Static)
        .expect("project context should be discovered");

    let scan = ProjectScanner::default().scan(&context, &reporter());

    let codes = scan
        .report()
        .iter()
        .filter_map(|diagnostic| diagnostic.code.as_deref())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"project::manifest::package"));
    assert!(codes.contains(&"project::source::inventory"));
    assert!(codes.contains(&"project::unsafe::inventory"));
    assert!(codes.contains(&"project::dependency::wildcard"));
    assert!(codes.contains(&"project::security::sensitive-file"));
    assert!(codes.contains(&"project::license::missing"));

    fs::remove_dir_all(root).expect("test project should clean up");
}

#[test]
fn project_scanner_uses_one_unified_report() {
    let root = temporary_project("report");

    write(
        &root,
        "Cargo.toml",
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"
license = "MIT"
"#,
    );

    write(&root, "src/lib.rs", "//! Demo crate.\n\npub fn demo() {}\n");

    write(&root, "LICENSE", "test license\n");
    write(&root, "README.md", "# Demo\n");

    let context = ProjectContext::discover(&root, ProjectScanProfile::Static)
        .expect("project context should be discovered");

    let scanner = ProjectScanner::default();

    let scan = scanner.scan(&context, &reporter());

    assert_eq!(scan.profile(), ProjectScanProfile::Static);
    assert_eq!(scan.analyzers_run(), scanner.analyzer_count());

    assert!(scan.files_scanned() >= 4);
    assert!(!scan.report().is_empty());

    scan.report()
        .digest()
        .expect("scan report should have canonical identity");

    fs::remove_dir_all(root).expect("test project should clean up");
}
