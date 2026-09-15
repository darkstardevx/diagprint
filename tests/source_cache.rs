use diagprint::{
    LabelKind, Reporter, SourceCache,
    render::{Renderer, TerminalRenderer},
};

fn renderer(width: usize) -> TerminalRenderer {
    TerminalRenderer {
        color: false,
        width,
        ..Default::default()
    }
}

#[test]
fn cache_is_shared_across_clones() {
    let cache = SourceCache::new();
    let clone = cache.clone();

    assert!(cache.is_empty());

    cache.insert("virtual://parser/input.tao", "let value = 42;\n");

    assert_eq!(cache.len(), 1);
    assert!(clone.contains("virtual://parser/input.tao"));

    clone.insert("virtual://parser/input.tao", "let value = 84;\n");

    let source = cache.get("virtual://parser/input.tao").unwrap();

    assert_eq!(&*source, "let value = 84;\n");
}

#[test]
fn virtual_source_renders_without_a_filesystem_file() {
    let cache = SourceCache::new();

    cache.insert(
        "virtual://parser/input.tao",
        concat!(
            "fn before() {}\n",
            "let value = broken();\n",
            "fn after() {}\n",
        ),
    );

    let reporter = Reporter::builder()
        .application("virtual-source-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("virtual source failed").label(
        "virtual://parser/input.tao",
        2,
        Some(5),
        Some(5),
        Some("cached value"),
    );

    let rendered = renderer(62).render_with_sources(&diagnostic, &cache);

    assert!(rendered.contains("virtual://parser/input.tao"));
    assert!(rendered.contains("let value = broken();"));
    assert!(rendered.contains("^^^^^"));
    assert!(rendered.contains("└─ cached value"));
}

#[test]
fn ordinary_render_does_not_magically_see_virtual_source() {
    let cache = SourceCache::new();

    cache.insert("virtual://memory/no-file.rs", "let cached = true;\n");

    let reporter = Reporter::builder()
        .application("virtual-source-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("virtual source").label(
        "virtual://memory/no-file.rs",
        1,
        Some(5),
        Some(6),
        Some("cached"),
    );

    let rendered = renderer(58).render(&diagnostic);

    assert!(rendered.contains("virtual://memory/no-file.rs"));
    assert!(!rendered.contains("let cached = true;"));

    let cached = renderer(58).render_with_sources(&diagnostic, &cache);

    assert!(cached.contains("let cached = true;"));
}

#[test]
fn cached_source_takes_precedence_over_disk() {
    let unique = format!(
        "diagprint-source-cache-{}-{}.rs",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let path = std::env::temp_dir().join(unique);

    std::fs::write(&path, "let disk_value = false;\n").unwrap();

    let path_string = path.to_string_lossy().into_owned();

    let cache = SourceCache::new();

    cache.insert(path_string.clone(), "let cached_value = true;\n");

    let reporter = Reporter::builder()
        .application("source-precedence-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("cache wins").label(
        path_string,
        1,
        Some(5),
        Some(12),
        Some("preferred source"),
    );

    let rendered = renderer(64).render_with_sources(&diagnostic, &cache);

    let _ = std::fs::remove_file(path);

    assert!(rendered.contains("cached_value"));
    assert!(!rendered.contains("disk_value"));
}

#[test]
fn reporter_builder_and_runtime_registration_share_cache() {
    let reporter = Reporter::builder()
        .application("reporter-cache-test")
        .source("virtual://initial.rs", "let initial = true;\n")
        .build()
        .unwrap();

    let cache = reporter.source_cache();

    assert!(cache.contains("virtual://initial.rs"));

    reporter.register_source("virtual://runtime.rs", "let runtime = true;\n");

    assert!(cache.contains("virtual://runtime.rs"));

    assert!(reporter.remove_source("virtual://initial.rs"));
    assert!(!cache.contains("virtual://initial.rs"));
}

#[test]
fn cached_render_preserves_secondary_label_role() {
    let cache = SourceCache::new();

    cache.insert("virtual://secondary.rs", "let secondary = value;\n");

    let reporter = Reporter::builder()
        .application("secondary-cache-test")
        .build()
        .unwrap();

    let diagnostic = reporter.warning("secondary source").secondary_label(
        "virtual://secondary.rs",
        1,
        Some(5),
        Some(9),
        Some("secondary value"),
    );

    assert_eq!(diagnostic.labels[0].kind, LabelKind::Secondary);

    let rendered = renderer(64).render_with_sources(&diagnostic, &cache);

    assert!(rendered.contains("::: secondary"));
    assert!(rendered.contains("---------"));
    assert!(rendered.contains("└· secondary value"));
}
