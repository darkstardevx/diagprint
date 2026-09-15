use diagprint::{
    CargoBuildSummary, CargoImportError, CargoMessage, CargoStreamImporter, CargoWorkspace,
    Reporter, Severity,
};
use serde_json::json;

const ROOT_ID: &str = "path+file:///workspace/demo#0.1.0";
const SERDE_ID: &str = "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.228";

fn metadata() -> String {
    json!({
        "packages": [
            {
                "name": "demo",
                "version": "0.1.0",
                "id": ROOT_ID,
                "source": null,
                "manifest_path": "/workspace/demo/Cargo.toml",
                "edition": "2024",
                "rust_version": "1.85",
                "documentation": null,
                "repository": "https://example.invalid/demo",
                "homepage": null,
                "targets": [
                    {
                        "kind": ["lib"],
                        "crate_types": ["lib"],
                        "name": "demo",
                        "src_path": "/workspace/demo/src/lib.rs",
                        "edition": "2024",
                        "required-features": ["runtime"],
                        "doc": true,
                        "doctest": true,
                        "test": true
                    }
                ]
            },
            {
                "name": "serde",
                "version": "1.0.228",
                "id": SERDE_ID,
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
        "workspace_members": [ROOT_ID],
        "workspace_default_members": [ROOT_ID],
        "resolve": {
            "nodes": [
                {
                    "id": ROOT_ID,
                    "dependencies": [SERDE_ID],
                    "deps": [
                        {
                            "name": "serde_alias",
                            "pkg": SERDE_ID,
                            "dep_kinds": [
                                {
                                    "kind": null,
                                    "target": null
                                },
                                {
                                    "kind": "dev",
                                    "target": "cfg(unix)"
                                }
                            ]
                        }
                    ],
                    "features": ["default", "runtime"]
                },
                {
                    "id": SERDE_ID,
                    "dependencies": [],
                    "deps": [],
                    "features": ["default"]
                }
            ],
            "root": ROOT_ID
        },
        "target_directory": "/workspace/target",
        "version": 1,
        "workspace_root": "/workspace"
    })
    .to_string()
}

fn target() -> serde_json::Value {
    json!({
        "kind": ["lib"],
        "crate_types": ["lib"],
        "name": "demo",
        "src_path": "/workspace/demo/src/lib.rs",
        "edition": "2024",
        "required-features": ["runtime"],
        "doc": true,
        "doctest": true,
        "test": true
    })
}

#[test]
fn metadata_preserves_package_and_workspace_identity() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let package = workspace.package(ROOT_ID).unwrap();

    assert_eq!(package.name, "demo");
    assert_eq!(package.version, "0.1.0");
    assert_eq!(package.edition, "2024");
    assert_eq!(package.rust_version.as_deref(), Some("1.85"));
    assert!(package.workspace_member);

    assert_eq!(
        workspace.root_package().map(|package| package.id.as_str()),
        Some(ROOT_ID)
    );

    assert_eq!(
        workspace.enabled_features(ROOT_ID),
        &["default".to_string(), "runtime".to_string()]
    );
}

#[test]
fn dependency_graph_preserves_renames_versions_and_kinds() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let dependencies = workspace.dependencies_of(ROOT_ID);

    assert_eq!(dependencies.len(), 1);

    let dependency = &dependencies[0];

    assert_eq!(dependency.name, "serde_alias");
    assert_eq!(dependency.package_id, SERDE_ID);
    assert_eq!(dependency.package_name.as_deref(), Some("serde"));
    assert_eq!(dependency.package_version.as_deref(), Some("1.0.228"));

    assert_eq!(dependency.kinds.len(), 2);

    assert_eq!(dependency.kinds[0].kind, None);
    assert_eq!(dependency.kinds[1].kind.as_deref(), Some("dev"));
    assert_eq!(dependency.kinds[1].target.as_deref(), Some("cfg(unix)"));
}

#[test]
fn metadata_rejects_unknown_format_version() {
    let raw = json!({
        "packages": [],
        "workspace_members": [],
        "target_directory": "/tmp/target",
        "workspace_root": "/tmp",
        "version": 99
    })
    .to_string();

    let error = CargoWorkspace::from_metadata_json(&raw).unwrap_err();

    assert!(matches!(
        error,
        CargoImportError::UnsupportedMetadataVersion { version: 99 }
    ));
}

#[test]
fn workspace_builds_exact_documentation_catalog() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let resolver = workspace.documentation_resolver();

    let link = resolver
        .crate_docs("serde", "trait.Deserialize.html")
        .unwrap();

    assert_eq!(
        link.url,
        "https://docs.rs/serde/1.0.228/serde/trait.Deserialize.html"
    );
}

#[test]
fn compiler_message_receives_cargo_context() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let importer = CargoStreamImporter::new().workspace(workspace);

    let reporter = Reporter::builder().build().unwrap();

    let raw = json!({
        "reason": "compiler-message",
        "package_id": ROOT_ID,
        "manifest_path": "/workspace/demo/Cargo.toml",
        "target": target(),
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
    })
    .to_string();

    let message = importer.import_line(&reporter, &raw).unwrap().unwrap();

    let CargoMessage::CompilerDiagnostic(message) = message else {
        panic!("expected compiler diagnostic");
    };

    assert_eq!(message.package.name.as_deref(), Some("demo"));
    assert_eq!(message.package.version.as_deref(), Some("0.1.0"));
    assert!(message.package.workspace_member);

    assert_eq!(message.dependencies.len(), 1);

    assert_eq!(message.diagnostic.severity, Severity::Error);
    assert_eq!(message.diagnostic.code.as_deref(), Some("E0277"));

    assert!(
        message
            .diagnostic
            .notes
            .iter()
            .any(|note| note == "cargo package: demo 0.1.0")
    );

    assert!(
        message
            .diagnostic
            .notes
            .iter()
            .any(|note| note.contains("cargo target: demo [lib] edition 2024"))
    );

    assert!(
        message
            .diagnostic
            .notes
            .iter()
            .any(|note| note == "cargo target required features: runtime")
    );

    assert!(
        message
            .diagnostic
            .notes
            .iter()
            .any(|note| note == "cargo resolved direct dependencies: 1")
    );
}

#[test]
fn artifact_message_preserves_profile_outputs_and_freshness() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let importer = CargoStreamImporter::new().workspace(workspace);

    let reporter = Reporter::builder().build().unwrap();

    let raw = json!({
        "reason": "compiler-artifact",
        "package_id": ROOT_ID,
        "manifest_path": "/workspace/demo/Cargo.toml",
        "target": target(),
        "profile": {
            "opt_level": "0",
            "debuginfo": 2,
            "debug_assertions": true,
            "overflow_checks": true,
            "test": false
        },
        "features": ["default", "runtime"],
        "filenames": [
            "/workspace/target/debug/libdemo.rlib",
            "/workspace/target/debug/deps/libdemo.rmeta"
        ],
        "executable": null,
        "fresh": true
    })
    .to_string();

    let message = importer.import_line(&reporter, &raw).unwrap().unwrap();

    let CargoMessage::Artifact(artifact) = message else {
        panic!("expected compiler artifact");
    };

    assert_eq!(artifact.package.name.as_deref(), Some("demo"));
    assert_eq!(artifact.profile.opt_level, "0");
    assert_eq!(artifact.profile.debuginfo, json!(2));
    assert!(artifact.profile.debug_assertions);
    assert!(artifact.fresh);
    assert_eq!(artifact.features, ["default", "runtime"]);
    assert_eq!(artifact.filenames.len(), 2);
}

#[test]
fn build_script_message_preserves_native_link_context() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let importer = CargoStreamImporter::new().workspace(workspace);

    let reporter = Reporter::builder().build().unwrap();

    let raw = json!({
        "reason": "build-script-executed",
        "package_id": ROOT_ID,
        "linked_libs": ["ssl", "static=z"],
        "linked_paths": ["native=/usr/lib"],
        "cfgs": ["has_native_tls"],
        "env": [
            ["OPENSSL_DIR", "/usr"]
        ],
        "out_dir": "/workspace/target/debug/build/demo/out"
    })
    .to_string();

    let message = importer.import_line(&reporter, &raw).unwrap().unwrap();

    let CargoMessage::BuildScript(script) = message else {
        panic!("expected build-script message");
    };

    assert_eq!(script.package.name.as_deref(), Some("demo"));
    assert_eq!(script.linked_libs, ["ssl", "static=z"]);
    assert_eq!(script.cfgs, ["has_native_tls"]);
    assert_eq!(
        script.env,
        [("OPENSSL_DIR".to_string(), "/usr".to_string())]
    );
    assert!(script.links_native_code());
}

#[test]
fn unknown_cargo_reason_is_preserved() {
    let importer = CargoStreamImporter::new();

    let reporter = Reporter::builder().build().unwrap();

    let raw = json!({
        "reason": "future-cargo-message",
        "future_field": 42
    })
    .to_string();

    let message = importer.import_line(&reporter, &raw).unwrap().unwrap();

    assert_eq!(message.reason(), "future-cargo-message");

    let CargoMessage::Unknown(message) = message else {
        panic!("expected unknown message");
    };

    assert_eq!(message.raw["future_field"], 42);
}

#[test]
fn non_json_output_is_ignored() {
    let importer = CargoStreamImporter::new();

    let reporter = Reporter::builder().build().unwrap();

    let message = importer
        .import_line(&reporter, "procedural macro says hello")
        .unwrap();

    assert!(message.is_none());
}

#[test]
fn build_summary_tracks_the_session() {
    let workspace = CargoWorkspace::from_metadata_json(&metadata()).unwrap();

    let importer = CargoStreamImporter::new().workspace(workspace);

    let reporter = Reporter::builder().build().unwrap();

    let lines = [
        json!({
            "reason": "compiler-message",
            "package_id": ROOT_ID,
            "manifest_path": "/workspace/demo/Cargo.toml",
            "target": target(),
            "message": {
                "$message_type": "diagnostic",
                "message": "warning from compiler",
                "code": null,
                "level": "warning",
                "spans": [],
                "children": [],
                "rendered": null
            }
        }),
        json!({
            "reason": "compiler-artifact",
            "package_id": ROOT_ID,
            "manifest_path": "/workspace/demo/Cargo.toml",
            "target": target(),
            "profile": {
                "opt_level": "0",
                "debuginfo": 2,
                "debug_assertions": true,
                "overflow_checks": true,
                "test": false
            },
            "features": [],
            "filenames": [],
            "executable": null,
            "fresh": true
        }),
        json!({
            "reason": "build-script-executed",
            "package_id": ROOT_ID,
            "linked_libs": ["ssl"],
            "linked_paths": [],
            "cfgs": [],
            "env": [],
            "out_dir": "/workspace/target/debug/build/demo/out"
        }),
        json!({
            "reason": "build-finished",
            "success": true
        }),
    ];

    let mut summary = CargoBuildSummary::default();

    for line in lines {
        let message = importer
            .import_line(&reporter, &line.to_string())
            .unwrap()
            .unwrap();

        summary.observe(&message);
    }

    assert_eq!(summary.compiler_diagnostics, 1);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.warnings, 1);

    assert_eq!(summary.artifacts, 1);
    assert_eq!(summary.fresh_artifacts, 1);

    assert_eq!(summary.build_scripts, 1);
    assert_eq!(summary.native_link_build_scripts, 1);

    assert_eq!(summary.package_count(), 1);
    assert_eq!(summary.successful(), Some(true));
}
