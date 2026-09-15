use codespan_reporting::{
    diagnostic::{Diagnostic, Label, Severity},
    files::SimpleFiles,
};
use diagprint::{CodespanDiagnosticExt, Reporter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = "fn main() {\n    let answer = 41;\n    println!(\"{answer}\");\n}\n";

    let mut files = SimpleFiles::new();

    let file_id = files.add("src/main.rs", source);

    let offset = source.find("41").expect("demo value exists");

    let diagnostic = Diagnostic {
        severity: Severity::Warning,

        code: Some("demo::answer".into()),

        message: "the answer is suspicious".into(),

        labels: vec![
            Label::primary(file_id, offset..offset + 2).with_message("expected 42"),
            Label::secondary(file_id, 0..2).with_message("inside this function"),
        ],

        notes: vec!["imported directly from codespan-reporting".into()],
    };

    let reporter = Reporter::builder().application("codespan-demo").build()?;

    let converted = diagnostic.to_diagprint(&reporter, &files);

    reporter.emit(&converted)?;

    Ok(())
}
