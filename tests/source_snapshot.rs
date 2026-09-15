use diagprint::{Reporter, SourceCache, render::TerminalRenderer};

fn renderer() -> TerminalRenderer {
    TerminalRenderer {
        color: false,
        width: 68,
        ..Default::default()
    }
}

#[test]
fn snapshot_keeps_original_source_after_live_update() {
    const NAME: &str = "memory://editor/main.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    cache.insert(NAME, "let value = new_value();\n");

    let reporter = Reporter::builder()
        .application("snapshot-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("stale editor diagnostic").label(
        NAME,
        1,
        Some(13),
        Some(9),
        Some("value at diagnostic time"),
    );

    let historical = renderer().render_with_snapshot(&diagnostic, &snapshot);

    let live = renderer().render_with_sources(&diagnostic, &cache);

    assert!(historical.contains("old_value"));
    assert!(!historical.contains("new_value"));

    assert!(live.contains("new_value"));
    assert!(!live.contains("old_value"));
}

#[test]
fn snapshot_survives_removal_from_live_cache() {
    const NAME: &str = "memory://generated/input.rs";

    let cache = SourceCache::new();

    cache.insert(NAME, "generated source\n");

    let snapshot = cache.snapshot();

    assert!(cache.remove(NAME).is_some());

    assert!(!cache.contains(NAME));
    assert!(snapshot.contains(NAME));

    assert_eq!(snapshot.get(NAME).as_deref(), Some("generated source\n"));
}

#[test]
fn snapshot_survives_live_cache_clear() {
    let cache = SourceCache::new();

    cache.insert("one.rs", "one\n");
    cache.insert("two.rs", "two\n");

    let snapshot = cache.snapshot();

    cache.clear();

    assert!(cache.is_empty());

    assert_eq!(snapshot.len(), 2);
    assert!(snapshot.contains("one.rs"));
    assert!(snapshot.contains("two.rs"));
}

#[test]
fn reporter_can_capture_source_snapshot() {
    const NAME: &str = "memory://reporter/main.rs";

    let reporter = Reporter::builder()
        .application("reporter-snapshot-test")
        .source(NAME, "let answer = 42;\n")
        .build()
        .unwrap();

    let snapshot = reporter.source_snapshot();

    reporter.register_source(NAME, "let answer = 84;\n");

    assert_eq!(snapshot.get(NAME).as_deref(), Some("let answer = 42;\n"));

    assert_eq!(
        reporter.source_cache().get(NAME).as_deref(),
        Some("let answer = 84;\n")
    );
}
