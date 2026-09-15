use diagprint::{
    render::{Renderer, TerminalRenderer},
    Applicability, DocumentationLink, Edit, Reporter, SuggestedCommand, Suggestion, TextRange,
};
use unicode_width::UnicodeWidthStr;

fn renderer(width: usize) -> TerminalRenderer {
    TerminalRenderer {
        color: false,
        width,
        ..Default::default()
    }
}

fn assert_box_width(rendered: &str, expected_width: usize) {
    for (index, line) in rendered.lines().enumerate() {
        assert_eq!(
            UnicodeWidthStr::width(line),
            expected_width,
            "line {} has incorrect width:\n{}",
            index + 1,
            line
        );
    }
}

#[test]
fn terminal_renderer_shows_structured_suggestion() {
    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter
        .error("chrono serde support is missing")
        .suggestion(
            Suggestion::new("Enable chrono serde")
                .explanation("DateTime serialization requires chrono's serde feature.")
                .applicability(Applicability::MachineApplicable)
                .documentation(DocumentationLink::docs_rs("chrono", "latest", "").language("rust"))
                .edit(Edit::replace(
                    "Cargo.toml",
                    TextRange::new(0, 3),
                    "old",
                    "new",
                ))
                .command(
                    SuggestedCommand::new("cargo check")
                        .explanation("Verify the project after applying the edit."),
                ),
        );

    let rendered = renderer(76).render(&diagnostic);

    assert!(rendered.contains("SUGGESTION"));
    assert!(rendered.contains("TITLE  Enable chrono serde"));
    assert!(rendered.contains("WHY"));
    assert!(rendered.contains("PATCH"));
    assert!(rendered.contains("- old"));
    assert!(rendered.contains("+ new"));
    assert!(rendered.contains("DOCS"));
    assert!(rendered.contains("docs.rs"));
    assert!(rendered.contains("COMMAND cargo check"));
    assert!(rendered.contains("machine-applicable"));
    assert!(rendered.contains("automatic fix available"));

    assert_box_width(&rendered, 76);
}

#[test]
fn manual_suggestion_reports_manual_review() {
    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("manual change needed").suggestion(
        Suggestion::new("Review configuration")
            .applicability(Applicability::Manual)
            .documentation(DocumentationLink::cargo_book("reference/features.html")),
    );

    let rendered = renderer(64).render(&diagnostic);

    assert!(rendered.contains("manual"));
    assert!(rendered.contains("manual review required"));

    assert_box_width(&rendered, 64);
}

#[test]
fn suggestion_rendering_is_ansi_free_when_color_is_disabled() {
    let reporter = Reporter::builder().build().unwrap();

    let diagnostic = reporter.error("problem").suggestion(
        Suggestion::new("Fix it")
            .applicability(Applicability::MachineApplicable)
            .edit(Edit::replace(
                "Cargo.toml",
                TextRange::new(0, 3),
                "old",
                "new",
            )),
    );

    let rendered = renderer(60).render(&diagnostic);

    assert!(!rendered.contains("\x1b["));
}
