use diagprint::{Applicability, Edit, FileCheck, FixPlan, TextRange};
use std::{fs, path::PathBuf};

fn replace(file: &PathBuf, content: &str, from: &str, to: &str) -> Edit {
    let start = content.find(from).expect("demo text exists");

    Edit::replace(file, TextRange::new(start, start + from.len()), from, to)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("target/diagprint-fixplan-demo");

    let manifest = root.join("Cargo.toml");

    let config = root.join("src/config.rs");

    fs::create_dir_all(config.parent().expect("config has a parent"))?;

    let manifest_original = "[features]\ndefault = []\n";

    let config_original = "pub const SERIALIZATION: bool = false;\n";

    fs::write(&manifest, manifest_original)?;

    fs::write(&config, config_original)?;

    let plan = FixPlan::new("Enable serialization support")
        .explanation("Update the feature declaration and runtime configuration together.")
        .applicability(Applicability::MachineApplicable)
        .precondition(FileCheck::equals(&manifest, manifest_original))
        .precondition(FileCheck::equals(&config, config_original))
        .edit(replace(
            &manifest,
            manifest_original,
            "default = []",
            "default = [\"serialization\"]",
        ))
        .edit(replace(&config, config_original, "false", "true"))
        .verify(FileCheck::contains(
            &manifest,
            "default = [\"serialization\"]",
        ))
        .verify(FileCheck::contains(&config, "SERIALIZATION: bool = true"))
        .backups(true);

    println!("{}", plan.preview().render());

    let check = plan.check()?;

    println!("ready: {} file(s) will change", check.affected_files.len());

    let report = plan.apply()?;

    println!(
        "applied: {} file(s), verified={}",
        report.changed_files.len(),
        report.verified()
    );

    Ok(())
}
