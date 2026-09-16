use diagprint::{
    DeltaPolicy, DiagnosticReport, ExportPolicy, GithubActionsDeltaRenderer, Reporter,
};

fn reporter() -> Reporter {
    Reporter::builder()
        .application("github-delta-test")
        .build()
        .unwrap()
}

fn report(diagnostics: impl IntoIterator<Item = diagprint::Diagnostic>) -> DiagnosticReport {
    diagnostics.into_iter().collect()
}

#[test]
fn default_renderer_emits_summary_new_and_changed_only() {
    let reporter = reporter();

    let baseline = report([
        reporter.warning("changed").code("G100"),
        reporter.error("resolved").code("G101"),
        reporter.info("persisting").code("G102"),
    ]);

    let candidate = report([
        reporter.error("changed").code("G100"),
        reporter.info("persisting").code("G102"),
        reporter.error("brand new").code("G103"),
    ]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let artifact = delta.evaluated_artifact(&DeltaPolicy::ci(), &ExportPolicy::default());

    let rendered = GithubActionsDeltaRenderer::new().render(&artifact);

    assert!(rendered.contains("title=diagprint delta"));
    assert!(rendered.contains("[changed] G100"));
    assert!(rendered.contains("[new] G103"));

    assert!(!rendered.contains("[resolved] G101"));
    assert!(!rendered.contains("[persisting] G102"));
}

#[test]
fn resolved_and_persisting_are_opt_in() {
    let reporter = reporter();

    let baseline = report([
        reporter.warning("resolved").code("G200"),
        reporter.info("persisting").code("G201"),
    ]);

    let candidate = report([reporter.info("persisting").code("G201")]);

    let delta = candidate.delta_from(&baseline).unwrap();
    let artifact = delta.artifact(&ExportPolicy::default());

    let rendered = GithubActionsDeltaRenderer::new()
        .with_resolved(true)
        .with_persisting(true)
        .render(&artifact);

    assert!(rendered.contains("[resolved] G200"));
    assert!(rendered.contains("[persisting] G201"));
}

#[test]
fn changed_annotation_shows_severity_transition() {
    let reporter = reporter();

    let baseline = report([reporter.warning("same logical diagnostic").code("G300")]);

    let candidate = report([reporter.error("same logical diagnostic").code("G300")]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let rendered = GithubActionsDeltaRenderer::new()
        .with_summary(false)
        .render(&delta.artifact(&ExportPolicy::default()));

    assert!(rendered.starts_with("::error"));
    assert!(rendered.contains("[changed] G300"));

    assert!(
        rendered.contains("severity%3A WARNING -> ERROR")
            || rendered.contains("severity: WARNING -> ERROR")
    );
}

#[test]
fn default_export_boundary_prevents_absolute_path_leak() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter.error("private path").code("G400").label(
        "/home/alice/private/project/src/main.rs",
        42,
        Some(3),
        Some(4),
        Some("bad value"),
    )]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let rendered =
        GithubActionsDeltaRenderer::new().render(&delta.artifact(&ExportPolicy::default()));

    assert!(rendered.contains("file=main.rs"));
    assert!(rendered.contains("line=42"));
    assert!(rendered.contains("col=3"));

    assert!(!rendered.contains("/home/alice/private/project/src/main.rs"));
}

#[test]
fn failing_ci_evaluation_is_presented_as_errors() {
    let reporter = reporter();

    let baseline = report([reporter.warning("promoted").code("G500")]);

    let candidate = report([
        reporter.error("promoted").code("G500"),
        reporter.error("new").code("G501"),
    ]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let artifact = delta.evaluated_artifact(&DeltaPolicy::ci(), &ExportPolicy::default());

    let rendered = GithubActionsDeltaRenderer::new().render(&artifact);

    assert!(rendered.contains("::error title=diagprint CI policy violation::"));

    assert!(rendered.contains("new_at_or_above"));
    assert!(rendered.contains("severity_increase_at_or_above"));
}

#[test]
fn github_command_payloads_are_escaped() {
    let reporter = reporter();

    let baseline = DiagnosticReport::new();

    let candidate = report([reporter
        .warning("value is 100%\nsecond line")
        .code("G:600,test")]);

    let delta = candidate.delta_from(&baseline).unwrap();

    let rendered = GithubActionsDeltaRenderer::new()
        .with_summary(false)
        .render(&delta.artifact(&ExportPolicy::default()));

    assert!(rendered.contains("G%3A600%2Ctest"));
    assert!(rendered.contains("100%25"));
    assert!(rendered.contains("%0Asecond line"));
}
