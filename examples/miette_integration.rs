use diagprint::{MietteReportExt, Reporter};
use miette::{LabeledSpan, MietteDiagnostic, NamedSource, Report, Severity};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn main() {\n    let answer = 41;\n}\n";

    let offset = source.find("41").expect("demo value exists");

    let diagnostic = MietteDiagnostic::new("the answer is incorrect")
        .with_code("demo::wrong_answer")
        .with_severity(Severity::Warning)
        .with_help("try the canonical answer")
        .with_url("https://example.com/docs/wrong-answer")
        .with_label(LabeledSpan::new(Some("expected 42".into()), offset, 2));

    let report = Report::new(diagnostic)
        .with_source_code(NamedSource::new("src/main.rs", source.to_owned()));

    let reporter = Reporter::builder().application("miette-demo").build()?;

    let tree = report.to_diagprint_tree(&reporter);

    reporter.emit(&tree.diagnostic)?;

    println!("related diagnostics: {}", tree.related_count());

    Ok(())
}
