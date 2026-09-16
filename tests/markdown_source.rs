use diagprint::{
    Reporter, SourceCache,
    render::{MarkdownRenderer, MarkdownSourceOptions, Renderer},
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("markdown-test")
        .color(false)
        .build()
        .expect("test reporter should build")
}

#[test]
fn ordinary_markdown_does_not_embed_source_implicitly() {
    let diagnostic = reporter().error("unused value").code("lint::unused").label(
        "src/main.rs",
        2,
        Some(9),
        Some(5),
        Some("unused here"),
    );

    let output = MarkdownRenderer.render(&diagnostic);

    assert!(output.contains("## Labels"));
    assert!(!output.contains("## Source"));
}

#[test]
fn source_aware_markdown_uses_language_tagged_rust_fence() {
    let sources = SourceCache::new();

    let _ = sources.insert(
        "src/main.rs",
        "fn main() {\n    let unused = 42;\n    println!(\"done\");\n}\n",
    );

    let diagnostic = reporter()
        .warning("unused value")
        .code("lint::unused")
        .label("src/main.rs", 2, Some(9), Some(6), Some("unused binding"));

    let output = MarkdownRenderer.render_with_sources(&diagnostic, &sources);

    assert!(output.contains("## Source"));
    assert!(output.contains("~~~rust"));
    assert!(output.contains("fn main() {"));
    assert!(output.contains("let unused = 42;"));
    assert!(output.contains("println!(\"done\");"));
    assert!(output.contains("Primary span"));
    assert!(output.contains("^^^^^^ unused binding"));
}

#[test]
fn markdown_source_context_is_configurable() {
    let sources = SourceCache::new();

    let _ = sources.insert(
        "src/lib.rs",
        "const ONE: u8 = 1;\n\
         const TWO: u8 = 2;\n\
         const THREE: u8 = 3;\n\
         const FOUR: u8 = 4;\n\
         const FIVE: u8 = 5;\n",
    );

    let diagnostic =
        reporter()
            .error("bad constant")
            .label("src/lib.rs", 3, Some(7), Some(5), Some("target"));

    let options = MarkdownSourceOptions::new().with_context_lines(1);

    let output = MarkdownRenderer.render_with_sources_and_options(&diagnostic, &sources, options);

    assert!(!output.contains("const ONE"));
    assert!(output.contains("const TWO"));
    assert!(output.contains("const THREE"));
    assert!(output.contains("const FOUR"));
    assert!(!output.contains("const FIVE"));
    assert!(output.contains("lines `2–4`"));
}

#[test]
fn unknown_source_types_fall_back_to_text() {
    let sources = SourceCache::new();

    let _ = sources.insert("virtual/input.weird", "alpha\nbeta\ngamma\n");

    let diagnostic = reporter().error("bad input").label(
        "virtual/input.weird",
        2,
        Some(1),
        Some(4),
        Some("bad token"),
    );

    let output = MarkdownRenderer.render_with_sources(&diagnostic, &sources);

    assert!(output.contains("~~~text"));
    assert!(output.contains("beta"));
}

#[test]
fn markdown_fence_expands_when_source_contains_tildes() {
    let sources = SourceCache::new();

    let _ = sources.insert("src/main.rs", "fn main() {\n    // ~~~ embedded fence\n}\n");

    let diagnostic = reporter().warning("embedded fence").label(
        "src/main.rs",
        2,
        Some(8),
        Some(3),
        Some("fence"),
    );

    let output = MarkdownRenderer.render_with_sources(&diagnostic, &sources);

    assert!(output.contains("~~~~rust"));
    assert!(output.contains("// ~~~ embedded fence"));
}

#[test]
fn stale_revision_is_reported_without_embedding_wrong_source() {
    let sources = SourceCache::new();

    let revision = sources.insert_revisioned("src/main.rs", "fn old_version() {}\n");

    let diagnostic = reporter().error("stale diagnostic").label_at_revision(
        "src/main.rs",
        revision,
        1,
        Some(4),
        Some(11),
        Some("old source"),
    );

    let _ = sources.insert("src/main.rs", "fn new_version() {}\n");

    let output = MarkdownRenderer.render_with_sources(&diagnostic, &sources);

    assert!(output.contains("Stale source revision"));
    assert!(!output.contains("fn old_version"));
    assert!(!output.contains("fn new_version"));
}

#[test]
fn secondary_labels_use_distinct_span_marker() {
    let sources = SourceCache::new();

    let _ = sources.insert("src/main.rs", "let first = 1;\nlet second = 2;\n");

    let diagnostic = reporter()
        .error("related values")
        .label("src/main.rs", 1, Some(5), Some(5), Some("primary"))
        .secondary_label("src/main.rs", 2, Some(5), Some(6), Some("secondary"));

    let output = MarkdownRenderer.render_with_sources(&diagnostic, &sources);

    assert!(output.contains("^^^^^ primary"));
    assert!(output.contains("------ secondary"));
}
