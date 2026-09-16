use diagprint::render::{GithubActionsRenderer, Renderer, SarifRenderer};
use diagprint::{ExportPath, ExportPolicy, ExportText, REDACTED, Reporter};
use serde_json::Value;
use std::path::PathBuf;

fn diagnostic() -> diagprint::Diagnostic {
    Reporter::builder()
        .application("external-policy-test")
        .build()
        .unwrap()
        .error("message-secret")
        .note("note-secret")
        .help("help-secret")
        .label(
            "/workspace/project/src/main.rs",
            4,
            Some(3),
            Some(5),
            Some("label-secret"),
        )
        .secondary_label(
            "/home/alice/private/lib.rs",
            8,
            Some(2),
            Some(3),
            Some("secondary-secret"),
        )
}

#[test]
fn github_actions_default_behavior_preserves_existing_paths() {
    let diagnostic = diagnostic();

    let rendered = GithubActionsRenderer.render(&diagnostic);

    assert!(rendered.contains("/workspace/project/src/main.rs"));
}

#[test]
fn github_actions_policy_uses_repository_relative_paths() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default().with_paths(ExportPath::RepositoryRelative(PathBuf::from(
        "/workspace/project",
    )));

    let rendered = GithubActionsRenderer.render_with_policy(&diagnostic, &policy);

    assert!(rendered.contains("file=src/main.rs"));

    assert!(rendered.contains("file=lib.rs"));

    assert!(!rendered.contains("/workspace/project"));

    assert!(!rendered.contains("/home/alice"));
}

#[test]
fn github_actions_policy_can_omit_locations_and_redact_text() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default()
        .with_paths(ExportPath::Omit)
        .with_text(ExportText::Redact);

    let rendered = GithubActionsRenderer.render_with_policy(&diagnostic, &policy);

    assert!(!rendered.contains("file="));

    assert!(rendered.contains(REDACTED));

    for secret in [
        "message-secret",
        "note-secret",
        "help-secret",
        "label-secret",
        "secondary-secret",
    ] {
        assert!(!rendered.contains(secret), "{secret} leaked",);
    }
}

#[test]
fn sarif_default_behavior_preserves_existing_paths() {
    let diagnostic = diagnostic();

    let rendered = SarifRenderer.render_many([&diagnostic]);

    assert!(rendered.contains("/workspace/project/src/main.rs"));
}

#[test]
fn sarif_policy_preserves_navigation_with_repository_relative_paths() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default().with_paths(ExportPath::RepositoryRelative(PathBuf::from(
        "/workspace/project",
    )));

    let rendered = SarifRenderer.render_many_with_policy([&diagnostic], &policy);

    let value: Value = serde_json::from_str(&rendered).unwrap();

    assert_eq!(
        value["runs"][0]
            ["results"][0]
            ["locations"][0]
            ["physicalLocation"]
            ["artifactLocation"]
            ["uri"]
            .as_str(),
        Some("src/main.rs"),
    );

    assert_eq!(
        value["runs"][0]
            ["results"][0]
            ["relatedLocations"][0]
            ["physicalLocation"]
            ["artifactLocation"]
            ["uri"]
            .as_str(),
        Some("lib.rs"),
    );

    assert!(!rendered.contains("/workspace/project"));

    assert!(!rendered.contains("/home/alice"));
}

#[test]
fn sarif_policy_can_omit_paths_and_redact_text() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default()
        .with_paths(ExportPath::Omit)
        .with_text(ExportText::Redact);

    let rendered = SarifRenderer.render_many_with_policy([&diagnostic], &policy);

    let value: Value = serde_json::from_str(&rendered).unwrap();

    assert!(
        value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]
            .get("artifactLocation")
            .is_none()
    );

    assert!(rendered.contains(REDACTED));

    for secret in [
        "message-secret",
        "note-secret",
        "help-secret",
        "label-secret",
        "secondary-secret",
    ] {
        assert!(!rendered.contains(secret), "{secret} leaked",);
    }
}
