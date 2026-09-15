use diagprint::{
    CargoBuildSummary, CargoMessage, CargoStreamImporter, CargoWorkspace, CompilerImporter,
    DocumentationResolver, Reporter,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let package_id = "path+file:///demo#0.1.0";
    let serde_id = "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.228";

    let metadata = json!({
        "packages": [
            {
                "name": "demo",
                "version": "0.1.0",
                "id": package_id,
                "source": null,
                "manifest_path": "/demo/Cargo.toml",
                "edition": "2024",
                "rust_version": "1.85",
                "documentation": null,
                "repository": null,
                "homepage": null,
                "targets": [
                    {
                        "kind": ["bin"],
                        "crate_types": ["bin"],
                        "name": "demo",
                        "src_path": "/demo/src/main.rs",
                        "edition": "2024",
                        "doc": true,
                        "doctest": false,
                        "test": true
                    }
                ]
            },
            {
                "name": "serde",
                "version": "1.0.228",
                "id": serde_id,
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "manifest_path": "/registry/serde/Cargo.toml",
                "edition": "2021",
                "rust_version": null,
                "documentation": null,
                "repository": null,
                "homepage": null,
                "targets": []
            }
        ],
        "workspace_members": [package_id],
        "workspace_default_members": [package_id],
        "resolve": {
            "nodes": [
                {
                    "id": package_id,
                    "dependencies": [serde_id],
                    "deps": [
                        {
                            "name": "serde",
                            "pkg": serde_id,
                            "dep_kinds": [
                                {
                                    "kind": null,
                                    "target": null
                                }
                            ]
                        }
                    ],
                    "features": ["default"]
                }
            ],
            "root": package_id
        },
        "target_directory": "/demo/target",
        "version": 1,
        "workspace_root": "/demo"
    });

    let workspace = CargoWorkspace::from_metadata_json(&metadata.to_string())?;

    let docs = workspace.documentation_resolver().rust_version("1.98.0");

    let compiler = CompilerImporter::new().documentation_resolver(docs);

    let importer = CargoStreamImporter::new()
        .compiler_importer(compiler)
        .workspace(workspace);

    let reporter = Reporter::builder()
        .application("diagprint-cargo")
        .width(88)
        .build()?;

    let messages = [
        json!({
            "reason": "compiler-message",
            "package_id": package_id,
            "manifest_path": "/demo/Cargo.toml",
            "target": {
                "kind": ["bin"],
                "crate_types": ["bin"],
                "name": "demo",
                "src_path": "/demo/src/main.rs",
                "edition": "2024",
                "doc": true,
                "doctest": false,
                "test": true
            },
            "message": {
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
            }
        }),
        json!({
            "reason": "compiler-artifact",
            "package_id": package_id,
            "manifest_path": "/demo/Cargo.toml",
            "target": {
                "kind": ["bin"],
                "crate_types": ["bin"],
                "name": "demo",
                "src_path": "/demo/src/main.rs",
                "edition": "2024",
                "doc": true,
                "doctest": false,
                "test": true
            },
            "profile": {
                "opt_level": "0",
                "debuginfo": 2,
                "debug_assertions": true,
                "overflow_checks": true,
                "test": false
            },
            "features": ["default"],
            "filenames": ["/demo/target/debug/demo"],
            "executable": "/demo/target/debug/demo",
            "fresh": false
        }),
        json!({
            "reason": "build-finished",
            "success": false
        }),
    ];

    let mut summary = CargoBuildSummary::default();

    for raw in messages {
        let Some(message) = importer.import_line(&reporter, &raw.to_string())? else {
            continue;
        };

        if let CargoMessage::CompilerDiagnostic(compiler) = &message {
            reporter.emit(&compiler.diagnostic)?;
        }

        summary.observe(&message);
    }

    println!(
        "cargo summary: diagnostics={} errors={} warnings={} artifacts={} packages={} success={:?}",
        summary.compiler_diagnostics,
        summary.errors,
        summary.warnings,
        summary.artifacts,
        summary.package_count(),
        summary.successful(),
    );

    let serde_docs =
        DocumentationResolver::crate_docs_at("serde", "1.0.228", "trait.Deserialize.html");

    println!("serde docs: {}", serde_docs.url);

    Ok(())
}
