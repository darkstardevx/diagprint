use diagprint::{DiagnosticHistory, GitProvenanceBinding, GitProvenanceRecord};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary_project(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-git-blame-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent should be created");
    }

    fs::write(path, contents).expect("file should be written");
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git should execute");

    assert!(
        output.status.success(),
        "git {:?} failed:\n{}",
        args,
        String::from_utf8_lossy(&output.stderr,),
    );

    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diagprint"))
        .args(args)
        .output()
        .expect("diagprint should execute")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout,),
        String::from_utf8_lossy(&output.stderr,),
    )
}

fn create_git_project(root: &Path) {
    write(
        root,
        "Cargo.toml",
        r#"[package]
name = "git-blame-demo"
version = "0.1.0"
edition = "2024"
license = "MIT"
"#,
    );

    write(
        root,
        "src/lib.rs",
        "//! Git provenance demo.\n\npub fn demo() {}\n",
    );

    write(root, "README.md", "# Git Blame Demo\n");

    write(root, "LICENSE", "test license\n");

    write(root, ".gitignore", ".diagprint/\n");

    git(root, &["init"]);

    git(root, &["config", "user.name", "Diagprint Test"]);

    git(root, &["config", "user.email", "diagprint@example.invalid"]);

    git(root, &["config", "commit.gpgsign", "false"]);

    git(root, &["add", "."]);

    git(root, &["commit", "-m", "baseline"]);
}

#[test]
fn captured_clean_scan_can_be_investigated_with_blame() {
    let root = temporary_project("captured");

    create_git_project(&root);

    let history = root.join(".diagprint/history");

    let capsule = root.join(".diagprint/capsule");

    let root_text = root.to_string_lossy().into_owned();

    let history_text = history.to_string_lossy().into_owned();

    let capsule_text = capsule.to_string_lossy().into_owned();

    let baseline = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--history-label",
        "baseline",
        "--git-provenance",
    ]);

    assert!(
        baseline.status.success(),
        "baseline provenance scan failed:\n{}",
        output_text(&baseline,),
    );

    write(
        &root,
        "src/lib.rs",
        "//! Git provenance demo.\n\npub unsafe fn raw_demo() {}\n",
    );

    git(&root, &["add", "src/lib.rs"]);

    git(&root, &["commit", "-m", "introduce unsafe api"]);

    let expected_commit = git(&root, &["rev-parse", "HEAD"]);

    let regression = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--history-label",
        "regression",
        "--git-provenance",
        "--capsule",
        &capsule_text,
    ]);

    assert!(
        regression.status.success(),
        "regression provenance scan failed:\n{}",
        output_text(&regression,),
    );

    let loaded = DiagnosticHistory::open(&history).expect("history should open");

    assert_eq!(loaded.len(), 2,);

    let fingerprint = loaded
        .fingerprints()
        .into_iter()
        .find(|fingerprint| loaded.lineage(fingerprint).first_seen_run() == Some(1))
        .expect("unsafe inventory should introduce a run-1 fingerprint");

    let short = fingerprint
        .rsplit(':')
        .next()
        .expect("fingerprint should contain digest");

    let short = &short[..12];

    let record = GitProvenanceRecord::load(&loaded, 1)
        .expect("record load should succeed")
        .expect("run 1 should have provenance");

    assert_eq!(record.binding, GitProvenanceBinding::CapturedClean,);

    assert_eq!(record.commit, expected_commit,);

    let blame = run(&[
        "blame",
        &history_text,
        short,
        "--run",
        "1",
        "--repo",
        &root_text,
    ]);

    assert!(
        blame.status.success(),
        "diagprint blame failed:\n{}",
        output_text(&blame,),
    );

    let blame_text = output_text(&blame);

    assert!(blame_text.contains("DIAGNOSTIC GIT PROVENANCE",),);

    assert!(blame_text.contains("binding: captured_clean",),);

    assert!(blame_text.contains("history-binding-verified: true",),);

    assert!(blame_text.contains("git-object-verified: true",),);

    assert!(blame_text.contains(&format!("commit: {expected_commit}"),),);

    assert!(blame_text.contains("subject: introduce unsafe api",),);

    assert!(blame_text.contains("src/lib.rs",),);

    assert!(blame_text.contains("causation: NOT ESTABLISHED",),);

    let provenance_path = capsule.join("provenance/project.json");

    let provenance: Value = serde_json::from_slice(
        &fs::read(provenance_path).expect("capsule provenance should exist"),
    )
    .expect("capsule provenance should parse");

    let expected_record_digest = record.record_digest.to_string();

    assert_eq!(
        provenance["attributes"]["git_provenance_record_digest"].as_str(),
        Some(expected_record_digest.as_str(),),
    );

    assert_eq!(
        provenance["attributes"]["git_commit"].as_str(),
        Some(expected_commit.as_str(),),
    );

    fs::remove_dir_all(root).expect("test should clean up");
}

#[test]
fn git_bind_creates_explicit_user_asserted_binding() {
    let root = temporary_project("asserted");

    create_git_project(&root);

    write(
        &root,
        "src/lib.rs",
        "//! Git provenance demo.\n\npub unsafe fn raw_demo() {}\n",
    );

    git(&root, &["add", "src/lib.rs"]);

    git(&root, &["commit", "-m", "unsafe snapshot"]);

    let commit = git(&root, &["rev-parse", "HEAD"]);

    let history = root.join(".diagprint/history");

    let root_text = root.to_string_lossy().into_owned();

    let history_text = history.to_string_lossy().into_owned();

    let scan = run(&["scan", &root_text, "--static", "--history", &history_text]);

    assert!(
        scan.status.success(),
        "scan failed:\n{}",
        output_text(&scan,),
    );

    let bind = run(&[
        "history",
        "git-bind",
        &history_text,
        "0",
        &commit,
        "--repo",
        &root_text,
    ]);

    assert!(
        bind.status.success(),
        "git-bind failed:\n{}",
        output_text(&bind,),
    );

    let bind_text = output_text(&bind);

    assert!(bind_text.contains("GIT PROVENANCE BOUND",),);

    assert!(bind_text.contains("binding: user_asserted",),);

    assert!(bind_text.contains("causation: NOT ESTABLISHED",),);

    let loaded = DiagnosticHistory::open(&history).expect("history should open");

    let record = GitProvenanceRecord::load(&loaded, 0)
        .expect("record should load")
        .expect("record should exist");

    assert_eq!(record.binding, GitProvenanceBinding::UserAsserted,);

    assert_eq!(record.commit, commit,);

    fs::remove_dir_all(root).expect("test should clean up");
}

#[test]
fn provenance_scan_rejects_dirty_worktree_before_history_append() {
    let root = temporary_project("dirty");

    create_git_project(&root);

    write(&root, "src/lib.rs", "//! dirty\n\npub fn modified() {}\n");

    let history = root.join(".diagprint/history");

    let root_text = root.to_string_lossy().into_owned();

    let history_text = history.to_string_lossy().into_owned();

    let scan = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--git-provenance",
    ]);

    assert!(!scan.status.success(), "dirty provenance scan should fail",);

    let text = output_text(&scan);

    assert!(text.contains("requires a clean Git worktree",),);

    assert!(
        !history.join("run-000000.json",).exists(),
        "failed provenance capture must not append history",
    );

    fs::remove_dir_all(root).expect("test should clean up");
}
