use diagprint::DiagnosticHistory;
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
        .expect("system clock should be after Unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "diagprint-history-cli-{name}-{}-{nonce}",
        std::process::id(),
    ))
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test parent should be created");
    }

    fs::write(path, contents).expect("test file should be written");
}

fn create_project(root: &Path) {
    write(
        root,
        "Cargo.toml",
        r#"[package]
name = "history-demo"
version = "0.1.0"
edition = "2024"
license = "MIT"
"#,
    );

    write(
        root,
        "src/lib.rs",
        "//! History demo crate.\n\npub fn demo() {}\n",
    );

    write(root, "README.md", "# History Demo\n");
    write(root, "LICENSE", "test license\n");
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diagprint"))
        .args(args)
        .output()
        .expect("diagprint binary should run")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn scan_history_verify_show_fingerprints_and_lineage_work_end_to_end() {
    let root = temporary_project("workflow");

    create_project(&root);

    let history = root.join(".diagprint/history");

    let root_text = root.to_string_lossy().into_owned();
    let history_text = history.to_string_lossy().into_owned();

    let first = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--history-label",
        "baseline",
    ]);

    assert!(
        first.status.success(),
        "first scan failed:\n{}",
        output_text(&first),
    );

    assert!(history.join("head.json").is_file());
    assert!(history.join("run-000000.json").is_file());

    write(
        &root,
        "src/lib.rs",
        "//! History demo crate.\n\npub unsafe fn raw_demo() {}\n",
    );

    let second = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--history-label",
        "candidate",
    ]);

    assert!(
        second.status.success(),
        "second scan failed:\n{}",
        output_text(&second),
    );

    assert!(history.join("run-000001.json").is_file());

    let verify = run(&["history", "verify", &history_text]);

    assert!(
        verify.status.success(),
        "history verify failed:\n{}",
        output_text(&verify),
    );

    let verify_text = output_text(&verify);

    assert!(verify_text.contains("HISTORY VERIFIED"));
    assert!(verify_text.contains("runs: 2"));
    assert!(verify_text.contains("chain-head: sha256:"));

    let show = run(&["history", "show", &history_text]);

    assert!(
        show.status.success(),
        "history show failed:\n{}",
        output_text(&show),
    );

    let show_text = output_text(&show);

    assert!(show_text.contains("DIAGPRINT HISTORY"));
    assert!(show_text.contains("runs: 2"));
    assert!(show_text.contains("baseline"));
    assert!(show_text.contains("candidate"));
    assert!(show_text.contains("run-digest: sha256:"));
    assert!(show_text.contains("delta:"));

    let fingerprints = run(&["history", "fingerprints", &history_text]);

    assert!(
        fingerprints.status.success(),
        "history fingerprints failed:\n{}",
        output_text(&fingerprints),
    );

    let fingerprints_text = output_text(&fingerprints);

    assert!(fingerprints_text.contains("DIAGPRINT HISTORY FINGERPRINTS"));
    assert!(fingerprints_text.contains("diagprint.canonical/v1:sha256:"));

    let loaded = DiagnosticHistory::open(&history).expect("history should reopen");

    let fingerprint = loaded
        .fingerprints()
        .into_iter()
        .next()
        .expect("history should contain a fingerprint");

    let hex = fingerprint
        .rsplit(':')
        .next()
        .expect("fingerprint should contain a digest");

    let short = &hex[..12];

    let lineage = run(&["history", "lineage", &history_text, short]);

    assert!(
        lineage.status.success(),
        "history lineage failed:\n{}",
        output_text(&lineage),
    );

    let lineage_text = output_text(&lineage);

    assert!(lineage_text.contains("DIAGPRINT LINEAGE"));
    assert!(lineage_text.contains(&fingerprint));

    let why = run(&["why", &history_text, short]);

    assert!(
        why.status.success(),
        "diagprint why failed:\n{}",
        output_text(&why),
    );

    let why_text = output_text(&why);

    assert!(why_text.contains("DIAGNOSTIC CASE FILE",),);

    assert!(why_text.contains("schema: diagprint.forensics.case-file/v1",),);

    assert!(why_text.contains(&fingerprint,),);

    assert!(why_text.contains("chain-verified: true",),);

    assert!(why_text.contains("EPISODES",),);

    assert!(why_text.contains("EVIDENCE",),);

    let history_why = run(&["history", "why", &history_text, short]);

    assert!(
        history_why.status.success(),
        "history why failed:\n{}",
        output_text(&history_why),
    );

    assert!(output_text(&history_why).contains("DIAGNOSTIC CASE FILE",),);

    fs::remove_dir_all(root).expect("test project should clean up");
}

#[test]
fn history_verify_rejects_tampered_run() {
    let root = temporary_project("tamper");

    create_project(&root);

    let history = root.join(".diagprint/history");

    let root_text = root.to_string_lossy().into_owned();
    let history_text = history.to_string_lossy().into_owned();

    let scan = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--history-label",
        "baseline",
    ]);

    assert!(
        scan.status.success(),
        "scan failed:\n{}",
        output_text(&scan),
    );

    let run_path = history.join("run-000000.json");

    let mut run_json: Value =
        serde_json::from_slice(&fs::read(&run_path).expect("history run should be readable"))
            .expect("history run should parse");

    run_json["label"] = Value::String("tampered".to_owned());

    fs::write(
        &run_path,
        serde_json::to_vec_pretty(&run_json).expect("tampered run should serialize"),
    )
    .expect("tampered run should be written");

    let verify = run(&["history", "verify", &history_text]);

    assert!(
        !verify.status.success(),
        "tampered history unexpectedly verified:\n{}",
        output_text(&verify),
    );

    assert!(
        output_text(&verify).contains("digest mismatch"),
        "unexpected verification failure:\n{}",
        output_text(&verify),
    );

    fs::remove_dir_all(root).expect("test project should clean up");
}

#[test]
fn capsule_anchors_history_chain_head() {
    let root = temporary_project("capsule-anchor");

    create_project(&root);

    let history = root.join(".diagprint/history");
    let capsule = root.join("evidence.diagpack");

    let root_text = root.to_string_lossy().into_owned();
    let history_text = history.to_string_lossy().into_owned();
    let capsule_text = capsule.to_string_lossy().into_owned();

    let scan = run(&[
        "scan",
        &root_text,
        "--static",
        "--history",
        &history_text,
        "--history-label",
        "anchored",
        "--capsule",
        &capsule_text,
    ]);

    assert!(
        scan.status.success(),
        "anchored scan failed:\n{}",
        output_text(&scan),
    );

    let loaded = DiagnosticHistory::open(&history).expect("history should verify");

    let head = loaded
        .head_digest()
        .expect("anchored history should have a chain head")
        .to_string();

    let latest = loaded.latest().expect("history should contain a run");

    let provenance_path = capsule.join("provenance/project.json");

    let provenance: Value = serde_json::from_slice(
        &fs::read(&provenance_path).expect("capsule provenance should be readable"),
    )
    .expect("capsule provenance should parse");

    let attributes = provenance["attributes"]
        .as_object()
        .expect("capsule provenance should contain attributes");

    assert_eq!(
        attributes.get("history_chain_head").and_then(Value::as_str),
        Some(head.as_str()),
    );

    assert_eq!(
        attributes.get("history_run_index").and_then(Value::as_str),
        Some("0"),
    );

    assert_eq!(
        attributes.get("history_run_count").and_then(Value::as_str),
        Some("1"),
    );

    assert_eq!(
        attributes.get("history_report").and_then(Value::as_str),
        Some(latest.report_digest.as_str()),
    );

    assert_eq!(
        attributes.get("history_schema").and_then(Value::as_str),
        Some("diagprint.history.run/v2"),
    );

    let verify_capsule = run(&["capsule", "verify", &capsule_text]);

    assert!(
        verify_capsule.status.success(),
        "anchored capsule failed verification:\n{}",
        output_text(&verify_capsule),
    );

    fs::remove_dir_all(root).expect("test project should clean up");
}

#[test]
fn history_lineage_rejects_unknown_fingerprint_prefix() {
    let root = temporary_project("unknown");

    fs::create_dir_all(&root).expect("history directory should be created");

    let root_text = root.to_string_lossy().into_owned();

    let output = run(&["history", "lineage", &root_text, "deadbeefdead"]);

    assert!(!output.status.success());

    assert!(output_text(&output).contains("no diagnostic fingerprint matches"),);

    fs::remove_dir_all(root).expect("test directory should clean up");
}
