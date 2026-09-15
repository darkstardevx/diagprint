use diagprint::{
    Applicability, DocumentationLink, Edit, Fixer, Reporter, SuggestedCommand, Suggestion,
    TextRange,
};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let demo_dir = PathBuf::from("target/diagprint-demo");
    let demo_file = demo_dir.join("Cargo.toml");

    fs::create_dir_all(&demo_dir)?;

    let original = "chrono = { version = \"0.4\", features = [\"clock\"] }";

    let replacement = "chrono = { version = \"0.4\", features = [\"clock\", \"serde\"] }";

    fs::write(&demo_file, format!("{original}\n"))?;

    let reporter = Reporter::builder()
        .application("diagprint-intelligence")
        .show_metadata(true)
        .width(88)
        .build()?;

    let diagnostic = reporter
        .error("chrono::DateTime cannot be serialized")
        .code("CARGO-001")
        .suggestion(
            Suggestion::new("Enable chrono's serde feature")
                .explanation(
                    "The chrono crate only provides serde implementations \
                     when its `serde` feature is enabled.",
                )
                .applicability(Applicability::MachineApplicable)
                .documentation(DocumentationLink::docs_rs("chrono", "latest", "").language("rust"))
                .documentation(DocumentationLink::cargo_book("reference/features.html"))
                .edit(Edit::replace(
                    &demo_file,
                    TextRange::new(0, original.len()),
                    original,
                    replacement,
                ))
                .command(
                    SuggestedCommand::new("cargo check")
                        .explanation("Verify the project after applying the edit."),
                ),
        );

    reporter.emit(&diagnostic)?;

    let fixer = Fixer::new().backups(true);

    match env::args().nth(1).as_deref() {
        Some("--check") => {
            let check = fixer.check(&diagnostic)?;

            println!();
            println!(
                "{} machine-applicable suggestion(s) validated.",
                check.applicable_suggestions
            );

            if check.affected_files.is_empty() {
                println!("No files would change.");
            } else {
                println!("Files that would change:");

                for file in check.affected_files {
                    println!("  {}", file.display());
                }
            }

            println!();
            println!("No files were modified.");
        }

        Some("--apply") => {
            let report = fixer.apply(&diagnostic)?;

            println!();
            println!("Applied {} suggestion(s).", report.applied_suggestions);

            if report.changed_files.is_empty() {
                println!("No files changed.");
            } else {
                for file in report.changed_files {
                    println!("Changed: {}", file.display());
                }
            }
        }

        Some("--interactive") => {
            fixer.apply_interactive(&diagnostic)?;
        }

        Some(argument) => {
            eprintln!("Unknown argument: {argument}");
            eprintln!();
            print_usage();
            std::process::exit(2);
        }

        None => {
            println!();
            println!("Preview only. No files were changed.");
            println!();
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("Try:");
    println!("  cargo run --example intelligence");
    println!("  cargo run --example intelligence -- --check");
    println!("  cargo run --example intelligence -- --apply");
    println!(
        "  cargo run --features terminal-docs \
         --example intelligence -- --interactive"
    );
}
