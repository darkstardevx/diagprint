use diagprint::{Reporter, render::SarifRenderer};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reporter = Reporter::builder().application("sarif-example").build()?;

    let type_error = reporter
        .error("Cannot combine incompatible values")
        .code("E-TYPE")
        .label("src/main.rs", 12, Some(9), Some(5), Some("numeric value"))
        .secondary_label(
            "src/lib.rs",
            4,
            Some(5),
            Some(8),
            Some("string declaration"),
        )
        .help("make both values use the same type");

    let warning = reporter
        .warning("Deprecated configuration")
        .code("W-CONFIG")
        .label(
            "src/config.rs",
            7,
            Some(1),
            Some(12),
            Some("deprecated setting"),
        );

    fs::create_dir_all("target")?;

    let path = "target/diagprint.sarif";

    SarifRenderer.write_many(path, [&type_error, &warning])?;

    println!("wrote {path}");

    Ok(())
}
