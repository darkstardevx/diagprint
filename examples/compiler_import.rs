use diagprint::{CompilerImporter, Fixer, Reporter};
use serde_json::json;
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("target/diagprint-compiler-demo");

    let source = root.join("src/main.rs");

    fs::create_dir_all(source.parent().expect("demo source has a parent"))?;

    let contents = "fn main() {\n    let unused = 42;\n}\n";

    fs::write(&source, contents)?;

    let start = contents.find("unused").expect("demo variable exists");

    let end = start + "unused".len();

    let rustc = json!({
        "$message_type": "diagnostic",
        "message": "unused variable: `unused`",
        "code": {
            "code": "unused_variables",
            "explanation": null
        },
        "level": "warning",
        "spans": [
            {
                "file_name": "src/main.rs",
                "byte_start": start,
                "byte_end": end,
                "line_start": 2,
                "line_end": 2,
                "column_start": 9,
                "column_end": 15,
                "is_primary": true,
                "text": [],
                "label": null,
                "suggested_replacement": null,
                "suggestion_applicability": null,
                "expansion": null
            }
        ],
        "children": [
            {
                "message": "if this is intentional, prefix it with an underscore",
                "code": null,
                "level": "help",
                "spans": [
                    {
                        "file_name": "src/main.rs",
                        "byte_start": start,
                        "byte_end": end,
                        "line_start": 2,
                        "line_end": 2,
                        "column_start": 9,
                        "column_end": 15,
                        "is_primary": true,
                        "text": [],
                        "label": null,
                        "suggested_replacement": "_unused",
                        "suggestion_applicability": "MachineApplicable",
                        "expansion": null
                    }
                ],
                "children": [],
                "rendered": null
            }
        ],
        "rendered": null
    });

    let reporter = Reporter::builder()
        .application("diagprint-compiler")
        .width(88)
        .build()?;

    let importer = CompilerImporter::new()
        .source_root(&root)
        .hydrate_edits(true);

    let diagnostic = importer
        .import_line(&reporter, &rustc.to_string())?
        .expect("rustc diagnostic");

    reporter.emit(&diagnostic)?;

    let check = Fixer::new().check(&diagnostic)?;

    println!(
        "compiler fixes currently applicable: {}",
        check.applicable_suggestions
    );

    println!("affected files: {:?}", check.affected_files);

    Ok(())
}
