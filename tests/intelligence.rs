use diagprint::{Applicability, DocumentationLink, Edit, Fixer, Reporter, Suggestion, TextRange};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn new(contents: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before UNIX epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "diagprint-intelligence-{}-{unique}.txt",
            std::process::id()
        ));

        fs::write(&path, contents).unwrap();

        Self { path }
    }

    fn read(&self) -> String {
        fs::read_to_string(&self.path).unwrap()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(format!("{}.diagprint.bak", self.path.display()));
    }
}

#[test]
fn suggestion_is_serialized_with_diagnostic() {
    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("missing serde feature").suggestion(
        Suggestion::new("Enable serde")
            .applicability(Applicability::Manual)
            .documentation(DocumentationLink::docs_rs("chrono", "latest", "")),
    );

    let json = serde_json::to_string(&diagnostic).unwrap();

    assert!(json.contains("Enable serde"));
    assert!(json.contains("documentation"));
}

#[test]
fn machine_applicable_edit_is_applied() {
    let file = TempFile::new("chrono = { version = \"0.4\", features = [\"clock\"] }\n");

    let original = "chrono = { version = \"0.4\", features = [\"clock\"] }";

    let replacement = "chrono = { version = \"0.4\", features = [\"clock\", \"serde\"] }";

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter
        .error("chrono serde support is missing")
        .suggestion(
            Suggestion::new("Enable chrono serde")
                .applicability(Applicability::MachineApplicable)
                .edit(Edit::replace(
                    &file.path,
                    TextRange::new(0, original.len()),
                    original,
                    replacement,
                )),
        );

    let report = Fixer::new().apply(&diagnostic).unwrap();

    assert_eq!(report.applied_suggestions, 1);
    assert_eq!(report.changed_files.len(), 1);
    assert!(file.read().contains("\"serde\""));
}

#[test]
fn manual_edit_is_not_auto_applied() {
    let file = TempFile::new("before\n");

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("manual suggestion").suggestion(
        Suggestion::new("Manual change")
            .applicability(Applicability::Manual)
            .edit(Edit::replace(
                &file.path,
                TextRange::new(0, 6),
                "before",
                "after",
            )),
    );

    let report = Fixer::new().apply(&diagnostic).unwrap();

    assert_eq!(report.applied_suggestions, 0);
    assert_eq!(file.read(), "before\n");
}

#[test]
fn stale_edit_is_refused() {
    let file = TempFile::new("actual\n");

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("stale suggestion").suggestion(
        Suggestion::new("Change text")
            .applicability(Applicability::MachineApplicable)
            .edit(Edit::replace(
                &file.path,
                TextRange::new(0, 6),
                "before",
                "after",
            )),
    );

    let result = Fixer::new().apply(&diagnostic);

    assert!(result.is_err());
    assert_eq!(file.read(), "actual\n");
}

#[test]
fn fixer_can_create_backup() {
    let file = TempFile::new("before\n");

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("change available").suggestion(
        Suggestion::new("Change text")
            .applicability(Applicability::MachineApplicable)
            .edit(Edit::replace(
                &file.path,
                TextRange::new(0, 6),
                "before",
                "after",
            )),
    );

    Fixer::new().backups(true).apply(&diagnostic).unwrap();

    let backup = fs::read_to_string(format!("{}.diagprint.bak", file.path.display())).unwrap();

    assert_eq!(backup, "before\n");
    assert_eq!(file.read(), "after\n");
}

#[test]
fn preview_contains_patch_and_docs() {
    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("feature missing").suggestion(
        Suggestion::new("Enable feature")
            .explanation("The crate requires serde support")
            .applicability(Applicability::MachineApplicable)
            .documentation(DocumentationLink::cargo_book("reference/features.html"))
            .edit(Edit::replace(
                "Cargo.toml",
                TextRange::new(0, 3),
                "old",
                "new",
            )),
    );

    let previews = Fixer::new().preview(&diagnostic);
    let rendered = previews[0].render();

    assert!(rendered.contains("PATCH Cargo.toml"));
    assert!(rendered.contains("- old"));
    assert!(rendered.contains("+ new"));
    assert!(rendered.contains("Cargo Book"));
    assert!(rendered.contains("automatic fix available"));
}
