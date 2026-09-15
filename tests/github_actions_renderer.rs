use diagprint::{
    Reporter,
    render::{GithubActionsRenderer, Renderer},
};

#[test]
fn maps_diagnostic_severity_to_actions_commands() {
    let reporter = Reporter::builder()
        .application("github-actions-test")
        .build()
        .unwrap();

    let cases = [
        (reporter.trace("trace"), "::notice::trace"),
        (reporter.debug("debug"), "::notice::debug"),
        (reporter.info("info"), "::notice::info"),
        (reporter.warning("warning"), "::warning::warning"),
        (reporter.error("error"), "::error::error"),
        (reporter.fatal("fatal"), "::error::fatal"),
    ];

    for (diagnostic, expected) in cases {
        assert_eq!(GithubActionsRenderer.render(&diagnostic), expected);
    }
}

#[test]
fn renders_primary_source_annotation() {
    let reporter = Reporter::builder()
        .application("github-actions-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("invalid value").code("E100").label(
        "src/main.rs",
        7,
        Some(5),
        Some(4),
        Some("value starts here"),
    );

    assert_eq!(
        GithubActionsRenderer.render(&diagnostic),
        concat!(
            "::error ",
            "file=src/main.rs,",
            "line=7,",
            "col=5,",
            "endColumn=8,",
            "title=E100",
            "::invalid value%0Avalue starts here",
        )
    );
}

#[test]
fn secondary_labels_render_as_notices() {
    let reporter = Reporter::builder()
        .application("github-actions-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("type mismatch")
        .code("E200")
        .label("src/main.rs", 3, Some(9), Some(5), Some("primary value"))
        .secondary_label(
            "src/lib.rs",
            9,
            Some(2),
            Some(3),
            Some("related declaration"),
        );

    let rendered = GithubActionsRenderer.render(&diagnostic);

    let mut lines = rendered.lines();

    assert_eq!(
        lines.next().unwrap(),
        concat!(
            "::error ",
            "file=src/main.rs,",
            "line=3,",
            "col=9,",
            "endColumn=13,",
            "title=E200",
            "::type mismatch%0Aprimary value",
        )
    );

    assert_eq!(
        lines.next().unwrap(),
        concat!(
            "::notice ",
            "file=src/lib.rs,",
            "line=9,",
            "col=2,",
            "endColumn=4,",
            "title=E200 related",
            "::related declaration",
        )
    );

    assert!(lines.next().is_none());
}

#[test]
fn escapes_workflow_command_data_and_properties() {
    let reporter = Reporter::builder()
        .application("github-actions-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .error("bad % value\nsecond line")
        .code("E:1,2")
        .label("C:\\src\\foo,bar.rs", 4, Some(3), None, None::<String>);

    let rendered = GithubActionsRenderer.render(&diagnostic);

    assert_eq!(
        rendered,
        concat!(
            "::error ",
            "file=C%3A\\src\\foo%2Cbar.rs,",
            "line=4,",
            "col=3,",
            "title=E%3A1%2C2",
            "::bad %25 value%0Asecond line",
        )
    );
}

#[test]
fn includes_notes_and_help_in_primary_annotation() {
    let reporter = Reporter::builder()
        .application("github-actions-test")
        .build()
        .unwrap();

    let diagnostic = reporter
        .warning("configuration issue")
        .note("first note")
        .note("second note")
        .help("update the configuration");

    assert_eq!(
        GithubActionsRenderer.render(&diagnostic),
        concat!(
            "::warning::configuration issue",
            "%0Anote: first note",
            "%0Anote: second note",
            "%0Ahelp: update the configuration",
        )
    );
}

#[test]
fn zero_length_does_not_emit_end_column() {
    let reporter = Reporter::builder()
        .application("github-actions-test")
        .build()
        .unwrap();

    let diagnostic = reporter.error("point diagnostic").label(
        "src/main.rs",
        1,
        Some(8),
        Some(0),
        None::<String>,
    );

    let rendered = GithubActionsRenderer.render(&diagnostic);

    assert!(rendered.contains("col=8"));
    assert!(!rendered.contains("endColumn="));
}
