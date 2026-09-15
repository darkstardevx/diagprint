use diagprint::render::{JsonRenderer, SarifRenderer};
use diagprint::{ExportPolicy, Reporter};
use serde_json::Value;

fn diagnostic() -> diagprint::Diagnostic {
    Reporter::builder()
        .application("fallible-renderer-test")
        .build()
        .unwrap()
        .error("example failure")
        .code("TEST-001")
}

#[test]
fn json_try_renderer_matches_compatibility_renderer() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default();

    let fallible = JsonRenderer
        .try_render_with_policy(&diagnostic, &policy)
        .unwrap();

    let compatibility = JsonRenderer.render_with_policy(&diagnostic, &policy);

    assert_eq!(fallible, compatibility);

    let _: Value = serde_json::from_str(&fallible).unwrap();
}

#[test]
fn json_try_report_renderer_matches_compatibility_renderer() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default();

    let fallible = JsonRenderer
        .try_render_report_with_policy([&diagnostic], &policy)
        .unwrap();

    let compatibility = JsonRenderer.render_report_with_policy([&diagnostic], &policy);

    assert_eq!(fallible, compatibility);

    let value: Value = serde_json::from_str(&fallible).unwrap();

    assert!(value.is_array());
}

#[test]
fn sarif_try_renderer_matches_compatibility_renderer() {
    let diagnostic = diagnostic();

    let fallible = SarifRenderer.try_render_many([&diagnostic]).unwrap();

    let compatibility = SarifRenderer.render_many([&diagnostic]);

    assert_eq!(fallible, compatibility);

    let value: Value = serde_json::from_str(&fallible).unwrap();

    assert_eq!(value["version"].as_str(), Some("2.1.0"),);
}

#[test]
fn sarif_try_policy_renderer_matches_compatibility_renderer() {
    let diagnostic = diagnostic();

    let policy = ExportPolicy::default();

    let fallible = SarifRenderer
        .try_render_many_with_policy([&diagnostic], &policy)
        .unwrap();

    let compatibility = SarifRenderer.render_many_with_policy([&diagnostic], &policy);

    assert_eq!(fallible, compatibility);
}
