use diagprint::{
    Reporter,
    render::{JsonRenderer, MarkdownRenderer, PlainRenderer, Renderer, TerminalRenderer},
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct TempSource {
    path: PathBuf,
}

impl TempSource {
    fn new(contents: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX epoch")
            .as_nanos();

        let path = std::env::temp_dir().join(format!(
            "diagprint-label-rendering-{}-{unique}.rs",
            std::process::id()
        ));

        fs::write(&path, contents).expect("failed to create temporary source file");

        Self { path }
    }

    fn path_string(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

impl Drop for TempSource {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn diagnostic() -> diagprint::Diagnostic {
    let reporter = Reporter::builder()
        .application("label-rendering-test")
        .build()
        .unwrap();

    reporter
        .error("two source locations")
        .label("src/main.rs", 3, Some(5), Some(4), Some("primary label"))
        .secondary_label("src/lib.rs", 8, Some(2), Some(6), Some("secondary label"))
}

#[test]
fn plain_renderer_identifies_primary_and_secondary_labels() {
    let rendered = PlainRenderer.render(&diagnostic());

    assert!(rendered.contains("  labels:"));
    assert!(rendered.contains("primary: src/main.rs:3:5 length=4 — primary label"));
    assert!(rendered.contains("secondary: src/lib.rs:8:2 length=6 — secondary label"));
}

#[test]
fn markdown_renderer_identifies_primary_and_secondary_labels() {
    let rendered = MarkdownRenderer.render(&diagnostic());

    assert!(rendered.contains("## Labels"));
    assert!(rendered.contains("- **Primary:** `src/main.rs:3:5` (length: `4`) — primary label"));
    assert!(rendered.contains("- **Secondary:** `src/lib.rs:8:2` (length: `6`) — secondary label"));
}

#[test]
fn json_renderer_keeps_primary_backward_compatible_and_secondary_explicit() {
    let rendered = JsonRenderer.render(&diagnostic());
    let value: Value = serde_json::from_str(&rendered).unwrap();

    let labels = value["labels"]
        .as_array()
        .expect("labels should serialize as an array");

    assert_eq!(labels.len(), 2);

    assert!(
        labels[0].get("kind").is_none(),
        "primary remains the backward-compatible default"
    );

    assert_eq!(
        labels[1].get("kind").and_then(Value::as_str),
        Some("secondary")
    );
}

#[test]
fn terminal_renderer_distinguishes_label_roles_without_color() {
    let source = TempSource::new("let alpha = one;\nlet beta = two;\n");

    let reporter = Reporter::builder()
        .application("label-rendering-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("two source locations")
        .label(
            source.path_string(),
            1,
            Some(5),
            Some(5),
            Some("primary label"),
        )
        .secondary_label(
            source.path_string(),
            2,
            Some(5),
            Some(4),
            Some("secondary label"),
        );

    let renderer = TerminalRenderer {
        color: false,
        width: 88,
        source_context_lines: 0,
        ..Default::default()
    };

    let rendered = renderer.render(&diagnostic);

    assert!(rendered.contains("--> primary "));
    assert!(rendered.contains("::: secondary "));

    assert!(rendered.contains("> 1 │ let alpha = one;"));
    assert!(rendered.contains(": 2 │ let beta = two;"));

    assert!(rendered.contains("^^^^^"));
    assert!(rendered.contains("----"));

    assert!(rendered.contains("└─ primary label"));
    assert!(rendered.contains("└· secondary label"));

    assert!(
        !rendered.contains("\u{1b}["),
        "semantic label distinction must not depend on ANSI color"
    );
}
