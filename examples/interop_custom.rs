use diagprint::{
    Cause, DocumentationLink, InteropDiagnostic, InteropDiagnosticSource,
    InteropDiagnosticSourceExt, InteropLabel, Reporter, Severity,
};

struct CustomLint {
    file: String,
}

impl InteropDiagnosticSource for CustomLint {
    fn to_interop_diagnostic(&self) -> InteropDiagnostic {
        InteropDiagnostic::new("configuration value is deprecated")
            .severity(Severity::Warning)
            .code("CUSTOM-001")
            .help("replace the deprecated configuration key")
            .note("reported by the custom lint engine")
            .label(
                InteropLabel::primary(&self.file, 2)
                    .column(1)
                    .length(10)
                    .message("deprecated key"),
            )
            .label(
                InteropLabel::secondary(&self.file, 4)
                    .column(1)
                    .length(8)
                    .message("related configuration"),
            )
            .cause(Cause::new("the configuration schema changed"))
            .documentation(DocumentationLink::new(
                "configuration migration guide",
                "https://example.com/migration",
            ))
            .related(
                InteropDiagnostic::new("another deprecated key was found").severity(Severity::Info),
            )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from("target/diagprint-interop-demo");

    std::fs::create_dir_all(&root)?;

    let file = root.join("config.toml");

    std::fs::write(&file, "enabled = true\nold_option = 1\n\nother = true\n")?;

    let lint = CustomLint {
        file: file.to_string_lossy().into_owned(),
    };

    let reporter = Reporter::builder().application("custom-lint").build()?;

    let tree = lint.to_diagprint_tree(&reporter);

    reporter.emit(&tree.diagnostic)?;

    println!(
        "diagnostic tree contains {} diagnostic(s)",
        tree.total_diagnostics()
    );

    Ok(())
}
