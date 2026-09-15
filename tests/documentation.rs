use diagprint::{CompilerImporter, DocumentationError, DocumentationResolver, Reporter};
use serde_json::json;
use std::{fs, path::PathBuf};
use uuid::Uuid;

fn temp_lock(name: &str, contents: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("diagprint-docs-{name}-{}", Uuid::now_v7()));

    fs::create_dir_all(&root).unwrap();

    let lock = root.join("Cargo.lock");

    fs::write(&lock, contents).unwrap();

    lock
}

#[test]
fn unique_locked_version_resolves_docs_rs() {
    let lock = temp_lock(
        "unique",
        r#"
version = 4

[[package]]
name = "serde"
version = "1.0.228"
"#,
    );

    let resolver = DocumentationResolver::from_cargo_lock(&lock).unwrap();

    let link = resolver
        .crate_docs("serde", "trait.Deserialize.html")
        .unwrap();

    assert_eq!(
        link.url,
        "https://docs.rs/serde/1.0.228/serde/trait.Deserialize.html"
    );

    fs::remove_dir_all(lock.parent().unwrap()).unwrap();
}

#[test]
fn hyphenated_package_uses_rustdoc_identifier() {
    let resolver = DocumentationResolver::new().with_package_version("foo-bar", "2.4.1");

    let link = resolver
        .crate_docs("foo-bar", "struct.Widget.html")
        .unwrap();

    assert_eq!(
        link.url,
        "https://docs.rs/foo-bar/2.4.1/foo_bar/struct.Widget.html"
    );
}

#[test]
fn custom_library_name_can_be_supplied() {
    let resolver = DocumentationResolver::new().with_package_version("package-name", "3.0.0");

    let link = resolver
        .crate_docs_with_name("package-name", "custom_lib", "index.html")
        .unwrap();

    assert_eq!(
        link.url,
        "https://docs.rs/package-name/3.0.0/custom_lib/index.html"
    );
}

#[test]
fn ambiguous_package_version_fails_closed() {
    let resolver = DocumentationResolver::new()
        .with_package_version("demo", "1.0.0")
        .with_package_version("demo", "2.0.0");

    let error = resolver.crate_docs("demo", "index.html").unwrap_err();

    match error {
        DocumentationError::AmbiguousPackage { package, versions } => {
            assert_eq!(package, "demo");

            assert_eq!(versions, vec!["1.0.0".to_string(), "2.0.0".to_string(),]);
        }

        other => {
            panic!("expected ambiguous package error, got {other:?}");
        }
    }
}

#[test]
fn unknown_package_does_not_fall_back_to_latest() {
    let resolver = DocumentationResolver::new();

    let error = resolver.crate_docs("mystery", "index.html").unwrap_err();

    assert!(matches!(error, DocumentationError::UnknownPackage { .. }));
}

#[test]
fn rust_version_pins_rust_owned_documentation() {
    let resolver = DocumentationResolver::new().rust_version("1.98.0");

    let error = resolver.rust_error("e0277");

    let std = resolver.rust_std("io/enum.ErrorKind.html");

    let rustc = resolver.rustc_json();

    let cargo = resolver.cargo_book("reference/manifest.html");

    assert_eq!(
        error.url,
        "https://doc.rust-lang.org/1.98.0/error_codes/E0277.html"
    );

    assert_eq!(
        std.url,
        "https://doc.rust-lang.org/1.98.0/std/io/enum.ErrorKind.html"
    );

    assert_eq!(
        rustc.url,
        "https://doc.rust-lang.org/1.98.0/rustc/json.html"
    );

    assert_eq!(
        cargo.url,
        "https://doc.rust-lang.org/1.98.0/cargo/reference/manifest.html"
    );
}

#[test]
fn compiler_importer_uses_configured_documentation_resolver() {
    let reporter = Reporter::builder().build().unwrap();

    let resolver = DocumentationResolver::new().rust_version("1.98.0");

    let importer = CompilerImporter::new().documentation_resolver(resolver);

    let line = json!({
        "$message_type": "diagnostic",
        "message": "trait bound is not satisfied",
        "code": {
            "code": "E0277",
            "explanation": null
        },
        "level": "error",
        "spans": [],
        "children": [],
        "rendered": null
    })
    .to_string();

    let diagnostic = importer.import_line(&reporter, &line).unwrap().unwrap();

    let link = diagnostic
        .suggestions
        .iter()
        .flat_map(|suggestion| suggestion.documentation.iter())
        .find(|link| link.label == "Rust error E0277")
        .expect("version-aware Rust error documentation");

    assert_eq!(
        link.url,
        "https://doc.rust-lang.org/1.98.0/error_codes/E0277.html"
    );
}
