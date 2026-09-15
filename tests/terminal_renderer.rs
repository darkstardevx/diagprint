use diagprint::{
    render::{Renderer, TerminalRenderer},
    Reporter,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use unicode_width::UnicodeWidthStr;

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
            "diagprint-terminal-test-{}-{unique}.rs",
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

fn renderer(width: usize) -> TerminalRenderer {
    TerminalRenderer {
        color: false,
        show_metadata: false,
        source_context_lines: 1,
        width,
    }
}

fn assert_box_width(rendered: &str, expected_width: usize) {
    for (index, line) in rendered.lines().enumerate() {
        let actual_width = UnicodeWidthStr::width(line);

        assert_eq!(
            actual_width,
            expected_width,
            "line {} has display width {}, expected {}\n{}",
            index + 1,
            actual_width,
            expected_width,
            line
        );
    }
}

#[test]
fn golden_basic_diagnostic_layout() {
    let reporter = Reporter::builder()
        .application("golden-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("Something failed")
        .code("E-GOLD")
        .cause("root cause")
        .note("remember this")
        .help("try again");

    let rendered = renderer(52).render(&diagnostic);

    let expected = concat!(
        "╭─ ✖ ERROR [E-GOLD] ───────────────────────────────╮\n",
        "│ Something failed                                 │\n",
        "│                                                  │\n",
        "│ CAUSE                                            │\n",
        "│ └─ root cause                                    │\n",
        "│                                                  │\n",
        "│ NOTE  remember this                              │\n",
        "│                                                  │\n",
        "│ HELP  try again                                  │\n",
        "╰──────────────────────────────────────────────────╯\n",
    );

    assert_eq!(rendered, expected);
}

#[test]
fn wraps_long_message_without_losing_text() {
    let reporter = Reporter::builder()
        .application("renderer-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error(
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda \
         mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega",
    );

    let rendered = renderer(48).render(&diagnostic);

    assert!(rendered.contains("alpha"));
    assert!(rendered.contains("omega"));
    assert_box_width(&rendered, 48);

    let message_rows = rendered
        .lines()
        .filter(|line| line.starts_with("│ "))
        .count();

    assert!(
        message_rows > 1,
        "expected the long diagnostic message to wrap"
    );
}

#[test]
fn unicode_text_keeps_box_alignment() {
    let reporter = Reporter::builder()
        .application("unicode-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error(
        "Unicode width test: 日本語テキスト Ελληνικά русский язык \
         with enough additional text to require wrapping.",
    );

    let rendered = renderer(54).render(&diagnostic);

    assert!(rendered.contains("日本語"));
    assert!(rendered.contains("русский"));
    assert_box_width(&rendered, 54);
}

#[test]
fn wraps_causes_notes_and_help_without_truncation() {
    let reporter = Reporter::builder()
        .application("wrapping-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("Top-level diagnostic")
        .cause(
            "This is a deliberately long underlying cause that should wrap \
             cleanly while preserving the entire cause message.",
        )
        .note(
            "This note is also deliberately long so that the renderer must \
             place part of it onto another terminal row.",
        )
        .help(
            "Check the configuration and verify that the final-important-word \
             remains visible after wrapping.",
        );

    let rendered = renderer(52).render(&diagnostic);

    assert!(rendered.contains("CAUSE"));
    assert!(rendered.contains("entire cause"));
    assert!(rendered.contains("another terminal"));
    assert!(rendered.contains("final-important-word"));
    assert_box_width(&rendered, 52);
}

#[test]
fn source_window_follows_highlight_on_long_line() {
    let long_line = format!("{}TARGET{}", "a".repeat(140), "b".repeat(90));

    let source = TempSource::new(&format!("fn before() {{}}\n{long_line}\nfn after() {{}}\n"));

    let reporter = Reporter::builder()
        .application("source-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("Long source line").label(
        source.path_string(),
        2,
        Some(141),
        Some(6),
        Some("focused label"),
    );

    let rendered = renderer(60).render(&diagnostic);

    assert!(
        rendered.contains("TARGET"),
        "highlighted text should remain visible:\n{rendered}"
    );

    assert!(
        rendered.contains('…'),
        "expected clipped source to contain an ellipsis:\n{rendered}"
    );

    assert!(
        rendered.contains("^^^^^^"),
        "expected six-character caret highlight:\n{rendered}"
    );

    assert!(
        rendered.contains("└─ focused label"),
        "expected source label text:\n{rendered}"
    );

    assert!(
        rendered.lines().any(|line| line.contains("> 2 │")),
        "expected target source line marker:\n{rendered}"
    );

    assert_box_width(&rendered, 60);
}

#[test]
fn long_source_label_wraps_beneath_highlight() {
    let source = TempSource::new(
        "fn before() {}\nlet configuration_value = load_configuration();\nfn after() {}\n",
    );

    let reporter = Reporter::builder()
        .application("label-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("Configuration failed").label(
        source.path_string(),
        2,
        Some(5),
        Some(19),
        Some(
            "this source label is intentionally long and should wrap \
             cleanly underneath the highlighted source region",
        ),
    );

    let rendered = renderer(58).render(&diagnostic);

    assert!(rendered.contains("^^^^^^^^^^^^^^^^^^^"));
    assert!(rendered.contains("└─ this source label"));
    assert!(rendered.contains("highlighted"));
    assert!(rendered.contains("source region"));

    assert_box_width(&rendered, 58);
}

#[test]
fn long_unbroken_word_wraps_instead_of_disappearing() {
    let reporter = Reporter::builder()
        .application("token-test")
        .build()
        .unwrap();

    let token = "diagnostic".repeat(20);
    let diagnostic = reporter.error(&token);

    let rendered = renderer(44).render(&diagnostic);

    assert_box_width(&rendered, 44);

    let reconstructed: String = rendered
        .lines()
        .filter(|line| line.starts_with("│ "))
        .map(|line| line.trim_start_matches("│ ").trim_end_matches(" │").trim())
        .collect();

    assert_eq!(
        reconstructed, token,
        "long unbroken token should survive wrapping"
    );
}
