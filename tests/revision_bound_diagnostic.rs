use diagprint::{
    Reporter, SourceCache,
    render::{JsonRenderer, Renderer, TerminalRenderer},
};

const NAME: &str = "memory://editor/revision-bound.rs";

fn renderer() -> TerminalRenderer {
    TerminalRenderer {
        color: false,
        width: 76,
        ..Default::default()
    }
}

#[test]
fn unrevisioned_diagnostic_keeps_legacy_json_shape() {
    let reporter = Reporter::builder()
        .application("revision-json-test")
        .build()
        .unwrap();

    let diagnostic =
        reporter
            .error("plain diagnostic")
            .label(NAME, 1, Some(5), Some(5), Some("plain label"));

    let json = JsonRenderer.render(&diagnostic);

    assert!(
        !json.contains("\"revision\""),
        "unrevisioned diagnostics should not gain revision JSON"
    );
}

#[test]
fn snapshot_binding_serializes_source_revision() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("revision-json-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("bound diagnostic")
        .label(NAME, 1, Some(13), Some(9), Some("bound value"))
        .bind_source_revisions(&snapshot);

    assert_eq!(diagnostic.labels[0].location.revision.unwrap().get(), 1);

    let json = JsonRenderer.render(&diagnostic);

    assert!(json.contains("\"revision\": 1"));
}

#[test]
fn revision_bound_diagnostic_detects_staleness() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("stale-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("editor diagnostic")
        .label(NAME, 1, Some(13), Some(9), Some("old value"))
        .bind_source_revisions(&snapshot);

    assert!(diagnostic.has_revisioned_sources());
    assert!(!diagnostic.has_stale_sources(&cache));

    cache.insert(NAME, "let value = new_value();\n");

    assert!(diagnostic.has_stale_sources(&cache));
}

#[test]
fn stale_live_source_is_not_rendered_as_valid_context() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("stale-render-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("editor diagnostic")
        .label(NAME, 1, Some(13), Some(9), Some("old value"))
        .bind_source_revisions(&snapshot);

    cache.insert(NAME, "let value = new_value();\n");

    let rendered = renderer().render_with_sources(&diagnostic, &cache);

    assert!(rendered.contains("stale source"));
    assert!(rendered.contains("! stale source: r1 != r2"));

    assert!(
        !rendered.contains("new_value"),
        "renderer must not underline newer source for an older diagnostic"
    );
}

#[test]
fn matching_snapshot_renders_original_source_normally() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("snapshot-render-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("editor diagnostic")
        .label(NAME, 1, Some(13), Some(9), Some("old value"))
        .bind_source_revisions(&snapshot);

    cache.insert(NAME, "let value = new_value();\n");

    let rendered = renderer().render_with_snapshot(&diagnostic, &snapshot);

    assert!(rendered.contains("old_value"));
    assert!(rendered.contains("^^^^^^^^^"));
    assert!(rendered.contains("└─ old value"));
    assert!(!rendered.contains("stale source"));
    assert!(!rendered.contains("new_value"));
}

#[test]
fn explicit_revision_builder_binds_one_label() {
    let cache = SourceCache::new();

    let revision = cache.insert_revisioned(NAME, "let value = 42;\n");

    let reporter = Reporter::builder()
        .application("explicit-revision-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("explicit revision").label_at_revision(
        NAME,
        revision,
        1,
        Some(5),
        Some(5),
        Some("value"),
    );

    assert_eq!(diagnostic.labels[0].location.revision, Some(revision));

    assert!(!diagnostic.has_stale_sources(&cache));
}
