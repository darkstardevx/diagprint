use diagprint::{Applicability, CompilerImporter, Edit, Fixer, Reporter, Severity};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

fn temp_project(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("diagprint-compiler-{name}-{}", Uuid::now_v7()))
}

fn write_source(root: &Path, contents: &str) -> PathBuf {
    let source = root.join("src/main.rs");

    fs::create_dir_all(source.parent().unwrap()).unwrap();

    fs::write(&source, contents).unwrap();

    source
}

fn rustc_diagnostic(
    file_name: &str,
    contents: &str,
    original: &str,
    replacement: &str,
    applicability: &str,
) -> String {
    let start = contents.find(original).unwrap();

    let end = start + original.len();

    json!({
        "$message_type": "diagnostic",
        "message": format!("problem with `{original}`"),
        "code": {
            "code": "E0277",
            "explanation": null
        },
        "level": "error",
        "spans": [
            {
                "file_name": file_name,
                "byte_start": start,
                "byte_end": end,
                "line_start": 2,
                "line_end": 2,
                "column_start": 9,
                "column_end": 9 + original.len(),
                "is_primary": true,
                "text": [],
                "label": "primary compiler span",
                "suggested_replacement": null,
                "suggestion_applicability": null,
                "expansion": null
            }
        ],
        "children": [
            {
                "message": "use the compiler suggestion",
                "code": null,
                "level": "help",
                "spans": [
                    {
                        "file_name": file_name,
                        "byte_start": start,
                        "byte_end": end,
                        "line_start": 2,
                        "line_end": 2,
                        "column_start": 9,
                        "column_end": 9 + original.len(),
                        "is_primary": true,
                        "text": [],
                        "label": null,
                        "suggested_replacement": replacement,
                        "suggestion_applicability": applicability,
                        "expansion": null
                    }
                ],
                "children": [],
                "rendered": null
            },
            {
                "message": "additional compiler context",
                "code": null,
                "level": "note",
                "spans": [],
                "children": [],
                "rendered": null
            }
        ],
        "rendered": null
    })
    .to_string()
}

#[test]
fn raw_rustc_diagnostic_preserves_structure() {
    let reporter = Reporter::builder().build().unwrap();

    let line = json!({
        "$message_type": "diagnostic",
        "message": "trait bound is not satisfied",
        "code": {
            "code": "E0277",
            "explanation": null
        },
        "level": "error",
        "spans": [
            {
                "file_name": "src/main.rs",
                "byte_start": 4,
                "byte_end": 8,
                "line_start": 7,
                "line_end": 7,
                "column_start": 5,
                "column_end": 9,
                "is_primary": true,
                "text": [],
                "label": "required here",
                "suggested_replacement": null,
                "suggestion_applicability": null,
                "expansion": null
            }
        ],
        "children": [
            {
                "message": "consider implementing the trait",
                "code": null,
                "level": "help",
                "spans": [],
                "children": [],
                "rendered": null
            },
            {
                "message": "required by this bound",
                "code": null,
                "level": "note",
                "spans": [],
                "children": [],
                "rendered": null
            }
        ],
        "rendered": null
    })
    .to_string();

    let diagnostic = CompilerImporter::new()
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    assert_eq!(diagnostic.severity, Severity::Error);

    assert_eq!(diagnostic.code.as_deref(), Some("E0277"));

    assert_eq!(
        diagnostic.help.as_deref(),
        Some("consider implementing the trait")
    );

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note == "required by this bound" })
    );

    assert_eq!(diagnostic.labels.len(), 1);

    assert!(diagnostic.suggestions.iter().any(|suggestion| {
        suggestion
            .documentation
            .iter()
            .any(|link| link.url.contains("/error_codes/E0277.html"))
    }));
}

#[test]
fn cargo_compiler_message_is_imported() {
    let reporter = Reporter::builder().build().unwrap();

    let line = json!({
        "reason": "compiler-message",
        "package_id": "path+file:///demo#0.1.0",
        "manifest_path": "/demo/Cargo.toml",
        "target": {
            "name": "demo"
        },
        "message": {
            "message": "unused variable",
            "code": {
                "code": "unused_variables",
                "explanation": null
            },
            "level": "warning",
            "spans": [],
            "children": [],
            "rendered": null
        }
    })
    .to_string();

    let diagnostic = CompilerImporter::new()
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    assert_eq!(diagnostic.severity, Severity::Warning);

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note.contains("cargo package:") })
    );

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note == "cargo target: demo" })
    );

    assert!(
        diagnostic
            .notes
            .iter()
            .any(|note| { note.contains("Cargo.toml") })
    );
}

#[test]
fn machine_applicable_replacement_becomes_guarded_edit() {
    let root = temp_project("machine");

    let contents = "fn main() {\n    let wrong = 42;\n}\n";

    let source = write_source(&root, contents);

    let line = rustc_diagnostic(
        "src/main.rs",
        contents,
        "wrong",
        "fixed",
        "MachineApplicable",
    );

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = CompilerImporter::new()
        .source_root(&root)
        .hydrate_edits(true)
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    let suggestion = diagnostic
        .suggestions
        .iter()
        .find(|suggestion| suggestion.has_edits())
        .expect("compiler edit");

    assert_eq!(suggestion.applicability, Applicability::MachineApplicable);

    assert_eq!(suggestion.edits.len(), 1);

    match &suggestion.edits[0] {
        Edit::Replace {
            expected,
            replacement,
            ..
        } => {
            assert_eq!(expected, "wrong");
            assert_eq!(replacement, "fixed");
        }

        other => {
            panic!("expected replacement edit, got {other:?}");
        }
    }

    let check = Fixer::new().check(&diagnostic).unwrap();

    assert_eq!(check.applicable_suggestions, 1);

    Fixer::new().apply(&diagnostic).unwrap();

    let updated = fs::read_to_string(&source).unwrap();

    assert!(updated.contains("let fixed = 42;"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hydration_is_disabled_by_default() {
    let root = temp_project("disabled");

    let contents = "fn main() {\n    let wrong = 42;\n}\n";

    write_source(&root, contents);

    let line = rustc_diagnostic(
        "src/main.rs",
        contents,
        "wrong",
        "fixed",
        "MachineApplicable",
    );

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = CompilerImporter::new()
        .source_root(&root)
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    let suggestion = diagnostic
        .suggestions
        .iter()
        .find(|suggestion| suggestion.title == "use the compiler suggestion")
        .unwrap();

    assert_eq!(suggestion.applicability, Applicability::Manual);

    assert!(suggestion.edits.is_empty());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_root_prevents_hydrating_outside_files() {
    let trusted = temp_project("trusted");

    let outside = temp_project("outside");

    fs::create_dir_all(&trusted).unwrap();

    let contents = "fn main() {\n    let wrong = 42;\n}\n";

    let source = write_source(&outside, contents);

    let line = rustc_diagnostic(
        source.to_str().unwrap(),
        contents,
        "wrong",
        "fixed",
        "MachineApplicable",
    );

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = CompilerImporter::new()
        .source_root(&trusted)
        .hydrate_edits(true)
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    let suggestion = diagnostic
        .suggestions
        .iter()
        .find(|suggestion| suggestion.title == "use the compiler suggestion")
        .unwrap();

    assert_eq!(suggestion.applicability, Applicability::Manual);

    assert!(suggestion.edits.is_empty());

    fs::remove_dir_all(trusted).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn zero_width_insert_is_not_machine_applicable() {
    let root = temp_project("insert");

    let contents = "fn main() {\n    let value = 42;\n}\n";

    write_source(&root, contents);

    let offset = contents.find("value").unwrap();

    let line = json!({
        "$message_type": "diagnostic",
        "message": "insertion suggested",
        "code": null,
        "level": "warning",
        "spans": [],
        "children": [
            {
                "message": "insert `mut `",
                "code": null,
                "level": "help",
                "spans": [
                    {
                        "file_name": "src/main.rs",
                        "byte_start": offset,
                        "byte_end": offset,
                        "line_start": 2,
                        "line_end": 2,
                        "column_start": 9,
                        "column_end": 9,
                        "is_primary": true,
                        "text": [],
                        "label": null,
                        "suggested_replacement": "mut ",
                        "suggestion_applicability": "MachineApplicable",
                        "expansion": null
                    }
                ],
                "children": [],
                "rendered": null
            }
        ],
        "rendered": null
    })
    .to_string();

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = CompilerImporter::new()
        .source_root(&root)
        .hydrate_edits(true)
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    let suggestion = diagnostic.suggestions.first().expect("compiler suggestion");

    assert_eq!(suggestion.applicability, Applicability::MaybeIncorrect);

    assert_eq!(suggestion.edits.len(), 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unknown_applicability_fails_closed() {
    let root = temp_project("unknown-applicability");

    let contents = "fn main() {\n    let wrong = 42;\n}\n";

    write_source(&root, contents);

    let line = rustc_diagnostic(
        "src/main.rs",
        contents,
        "wrong",
        "fixed",
        "FutureRustcApplicability",
    );

    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = CompilerImporter::new()
        .source_root(&root)
        .hydrate_edits(true)
        .import_line(&reporter, &line)
        .unwrap()
        .unwrap();

    let suggestion = diagnostic
        .suggestions
        .iter()
        .find(|suggestion| suggestion.has_edits())
        .expect("compiler suggestion");

    assert_eq!(suggestion.applicability, Applicability::Manual);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn non_json_lines_are_ignored() {
    let reporter = Reporter::builder().build().unwrap();

    let result = CompilerImporter::new()
        .import_line(&reporter, "procedural macro said hello")
        .unwrap();

    assert!(result.is_none());
}

#[test]
fn unrelated_cargo_messages_are_ignored() {
    let reporter = Reporter::builder().build().unwrap();

    let line = json!({
        "reason": "build-finished",
        "success": true
    })
    .to_string();

    let result = CompilerImporter::new()
        .import_line(&reporter, &line)
        .unwrap();

    assert!(result.is_none());
}
