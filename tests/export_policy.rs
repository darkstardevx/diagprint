use diagprint::render::JsonRenderer;
use diagprint::{
    DiagnosticSink, ExportAttributes, ExportPath, ExportPolicy, ExportRemediation, ExportText,
    ExportUrl, JsonLinesSink, REDACTED, Reporter, SuggestedCommand, Suggestion,
};
use serde_json::Value;
use std::path::PathBuf;

fn reporter() -> Reporter {
    Reporter::builder()
        .application("export-policy-test")
        .build()
        .unwrap()
}

#[test]
fn default_policy_matches_safe_export_contract() {
    let diagnostic = reporter()
        .error("visible message")
        .attribute("authorization", "Bearer secret")
        .label(
            "/home/alice/project/src/main.rs",
            7,
            Some(3),
            Some(4),
            Some("visible label"),
        );

    let rendered = JsonRenderer.render_with_policy(&diagnostic, &ExportPolicy::default());

    let value: Value = serde_json::from_str(&rendered).unwrap();

    assert_eq!(value["message"].as_str(), Some("visible message"),);

    assert_eq!(value["application"].as_str(), Some("export-policy-test"),);

    assert!(value.get("pid").is_none());

    assert!(value.get("hostname").is_none());

    assert!(value.get("attributes").is_none());

    assert_eq!(value["attribute_count"].as_u64(), Some(1),);

    assert_eq!(
        value["labels"][0]["location"]["file"].as_str(),
        Some("main.rs"),
    );
}

#[test]
fn repository_relative_paths_preserve_navigation_without_leaking_outside_paths() {
    let diagnostic = reporter()
        .error("path test")
        .label(
            "/workspace/project/src/main.rs",
            3,
            None,
            None,
            None::<String>,
        )
        .secondary_label(
            "/home/alice/private/secret.rs",
            9,
            None,
            None,
            None::<String>,
        );

    let policy = ExportPolicy::default().with_paths(ExportPath::RepositoryRelative(PathBuf::from(
        "/workspace/project",
    )));

    let rendered = JsonRenderer.render_with_policy(&diagnostic, &policy);

    let value: Value = serde_json::from_str(&rendered).unwrap();

    assert_eq!(
        value["labels"][0]["location"]["file"].as_str(),
        Some("src/main.rs"),
    );

    assert_eq!(
        value["labels"][1]["location"]["file"].as_str(),
        Some("secret.rs"),
    );

    assert!(!rendered.contains("/home/alice"));
}

#[test]
fn sensitive_attribute_filter_preserves_non_sensitive_types() {
    let diagnostic = reporter()
        .warning("attribute test")
        .attribute("authorization", "Bearer super-secret")
        .attribute("user_id", 42_u64)
        .attribute("cache_hit", true);

    let policy = ExportPolicy::default().with_attributes(ExportAttributes::RedactSensitive);

    let rendered = JsonRenderer.render_with_policy(&diagnostic, &policy);

    let value: Value = serde_json::from_str(&rendered).unwrap();

    let attributes = value["attributes"]
        .as_array()
        .expect("attributes should exist");

    assert_eq!(attributes[0]["name"].as_str(), Some("authorization"),);

    assert_eq!(attributes[0]["value"].as_str(), Some(REDACTED),);

    assert_eq!(attributes[1]["name"].as_str(), Some("user_id"),);

    assert_eq!(attributes[1]["value"].as_u64(), Some(42),);

    assert_eq!(attributes[2]["value"].as_bool(), Some(true),);

    assert!(!rendered.contains("Bearer super-secret"));
}

#[test]
fn text_and_remediation_can_be_reduced_without_exposing_payloads() {
    let diagnostic = reporter()
        .error("message-secret")
        .help("help-secret")
        .note("note-secret")
        .cause("cause-secret")
        .suggestion(
            Suggestion::new("suggestion-secret")
                .command(SuggestedCommand::new("tool --token command-secret")),
        );

    let policy = ExportPolicy::default()
        .with_text(ExportText::Redact)
        .with_remediation(ExportRemediation::Omit);

    let rendered = JsonRenderer.render_with_policy(&diagnostic, &policy);

    assert!(rendered.contains(REDACTED));

    for secret in [
        "message-secret",
        "help-secret",
        "note-secret",
        "cause-secret",
        "suggestion-secret",
        "command-secret",
    ] {
        assert!(!rendered.contains(secret), "{secret} leaked",);
    }

    let value: Value = serde_json::from_str(&rendered).unwrap();

    assert!(value.get("suggestions").is_none());
}

#[test]
fn url_policy_requires_explicit_opt_in_for_unsanitized_urls() {
    let diagnostic =
        reporter()
            .info("URL test")
            .suggestion(Suggestion::new("read docs").documentation(
                diagprint::DocumentationLink::new(
                    "private docs",
                    "https://alice:password@example.com/docs?token=secret#private",
                ),
            ));

    let safe = JsonRenderer.render_with_policy(&diagnostic, &ExportPolicy::default());

    assert!(safe.contains("https://example.com/docs"));

    assert!(!safe.contains("alice:password"));

    assert!(!safe.contains("token=secret"));

    let omitted = JsonRenderer.render_with_policy(
        &diagnostic,
        &ExportPolicy::default().with_urls(ExportUrl::Omit),
    );

    let omitted: Value = serde_json::from_str(&omitted).unwrap();

    assert!(
        omitted["suggestions"][0]["documentation"][0]
            .get("url")
            .is_none()
    );

    let full = JsonRenderer.render_with_policy(
        &diagnostic,
        &ExportPolicy::default().with_urls(ExportUrl::Full),
    );

    assert!(full.contains("alice:password"));

    assert!(full.contains("token=secret"));
}

#[test]
fn json_lines_sink_uses_configured_policy() {
    let diagnostic = reporter()
        .warning("request failed")
        .attribute("authorization", "Bearer secret")
        .attribute("attempt", 3_u64);

    let policy = ExportPolicy::default().with_attributes(ExportAttributes::RedactSensitive);

    let sink = JsonLinesSink::with_policy(Vec::<u8>::new(), policy);

    sink.emit(&diagnostic).unwrap();

    let rendered = String::from_utf8(sink.into_inner().unwrap()).unwrap();

    assert!(rendered.contains("\"authorization\""));

    assert!(rendered.contains(REDACTED));

    assert!(rendered.contains("\"attempt\""));

    assert!(!rendered.contains("Bearer secret"));
}
