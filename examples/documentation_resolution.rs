use diagprint::DocumentationResolver;
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("target/diagprint-docs-demo");

    fs::create_dir_all(&root)?;

    let lock = root.join("Cargo.lock");

    fs::write(
        &lock,
        r#"
version = 4

[[package]]
name = "serde"
version = "1.0.228"

[[package]]
name = "foo-bar"
version = "2.4.1"
"#,
    )?;

    let resolver = DocumentationResolver::from_cargo_lock(&lock)?.rust_version("1.98.0");

    let serde = resolver.crate_docs("serde", "trait.Deserialize.html")?;

    let hyphenated = resolver.crate_docs("foo-bar", "struct.Widget.html")?;

    let rust_error = resolver.rust_error("E0277");

    println!("serde: {}", serde.url);

    println!("hyphenated crate: {}", hyphenated.url);

    println!("rust error: {}", rust_error.url);

    Ok(())
}
