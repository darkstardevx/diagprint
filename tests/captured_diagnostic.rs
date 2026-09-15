use diagprint::{CapturedDiagnostic, Reporter, SourceCache, render::TerminalRenderer};

const NAME: &str = "memory://editor/captured.rs";

fn renderer() -> TerminalRenderer {
    TerminalRenderer {
        color: false,
        width: 76,
        ..Default::default()
    }
}

#[test]
fn captured_diagnostic_binds_snapshot_revision() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("captured-test")
        .build()
        .unwrap();

    let diagnostic =
        reporter
            .error("captured diagnostic")
            .label(NAME, 1, Some(13), Some(9), Some("value"));

    let captured = CapturedDiagnostic::new(diagnostic, snapshot);

    assert_eq!(
        captured.diagnostic().labels[0]
            .location
            .revision
            .unwrap()
            .get(),
        1
    );
}

#[test]
fn captured_diagnostic_detects_live_staleness() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("captured-test")
        .build()
        .unwrap();

    let captured = CapturedDiagnostic::new(
        reporter
            .error("captured diagnostic")
            .label(NAME, 1, Some(13), Some(9), Some("value")),
        snapshot,
    );

    assert!(captured.is_current(&cache));
    assert!(!captured.is_stale(&cache));

    cache.insert(NAME, "let value = new_value();\n");

    assert!(!captured.is_current(&cache));
    assert!(captured.is_stale(&cache));
}

#[test]
fn captured_render_uses_original_source_after_live_update() {
    let cache = SourceCache::new();

    cache.insert(NAME, "let value = old_value();\n");

    let snapshot = cache.snapshot();

    let reporter = Reporter::builder()
        .application("captured-test")
        .build()
        .unwrap();

    let captured = CapturedDiagnostic::new(
        reporter.error("captured diagnostic").label(
            NAME,
            1,
            Some(13),
            Some(9),
            Some("original value"),
        ),
        snapshot,
    );

    cache.insert(NAME, "let value = new_value();\n");

    let rendered = renderer().render_captured(&captured);

    assert!(rendered.contains("old_value"));
    assert!(rendered.contains("original value"));

    assert!(!rendered.contains("new_value"));
    assert!(!rendered.contains("stale source"));
}

#[test]
fn reporter_capture_uses_reporter_source_snapshot() {
    let reporter = Reporter::builder()
        .application("reporter-capture-test")
        .source(NAME, "let value = original();\n")
        .build()
        .unwrap();

    let diagnostic =
        reporter
            .error("captured by reporter")
            .label(NAME, 1, Some(13), Some(8), Some("original"));

    let captured = reporter.capture(diagnostic);

    assert_eq!(
        captured.diagnostic().labels[0]
            .location
            .revision
            .unwrap()
            .get(),
        1
    );

    reporter.register_source(NAME, "let value = changed();\n");

    assert!(captured.is_stale(&reporter.source_cache()));

    let rendered = renderer().render_captured(&captured);

    assert!(rendered.contains("original()"));
    assert!(!rendered.contains("changed()"));
}

#[test]
fn into_parts_preserves_diagnostic_and_snapshot() {
    let cache = SourceCache::new();

    cache.insert(NAME, "source\n");

    let reporter = Reporter::builder()
        .application("parts-test")
        .build()
        .unwrap();

    let captured = CapturedDiagnostic::new(
        reporter.error("parts").source(NAME, 1, None),
        cache.snapshot(),
    );

    let (diagnostic, snapshot) = captured.into_parts();

    assert_eq!(diagnostic.message, "parts");
    assert_eq!(snapshot.get(NAME).as_deref(), Some("source\n"));
}
