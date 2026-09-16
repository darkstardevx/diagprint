use diagprint::{
    CapsuleProvenance, DiagnosticCapsule, DiagnosticCapsuleManifest, DiagnosticReport, Reporter,
    project_scan::{
        ProjectContext, ProjectScanProfile, ProjectScanner, ProjectTool, ProjectToolOutput,
    },
    render::{
        AuditTranscriptRenderer, CompilerTextRenderer, MarkdownRenderer, PlainRenderer,
        ReportRenderer,
    },
};

#[cfg(feature = "html")]
use diagprint::render::HtmlRenderer;

use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{self, Command},
};

#[derive(Debug, Clone, Copy, Default)]
enum OutputFormat {
    #[default]
    Compiler,
    Plain,
    Audit,
    Markdown,
    Html,
}

#[derive(Debug)]
struct ScanArgs {
    path: PathBuf,
    profile: ProjectScanProfile,
    format: OutputFormat,
    output: Option<PathBuf>,
    capsule: Option<PathBuf>,
}

fn main() {
    match run() {
        Ok(exit_code) => {
            process::exit(exit_code);
        }

        Err(error) => {
            eprintln!("diagprint: {error}");
            process::exit(2);
        }
    }
}

fn run() -> Result<i32, Box<dyn Error>> {
    let mut args = env::args().skip(1);

    let Some(command) = args.next() else {
        print_usage();
        return Ok(2);
    };

    match command.as_str() {
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(0)
        }

        "scan" => {
            let scan_args = parse_scan_args(args.collect())?;

            run_scan(scan_args)
        }

        "capsule" => run_capsule_command(args.collect()),

        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command {other:?}; expected `scan` or `capsule`"),
        )
        .into()),
    }
}

fn run_scan(scan_args: ScanArgs) -> Result<i32, Box<dyn Error>> {
    if scan_args.profile == ProjectScanProfile::Deep {
        eprintln!("diagprint scan: deep profile explicitly enabled");

        eprintln!(
            "diagprint scan: Cargo check, Clippy, build scripts, proc macros, and tests may execute project code"
        );
    }

    let reporter = Reporter::builder()
        .application("diagprint-scan")
        .color(false)
        .build()?;

    let mut context = ProjectContext::discover(&scan_args.path, scan_args.profile)?;

    collect_external_evidence(&mut context);

    let scanner = ProjectScanner::with_builtin_analyzers();

    let scan = scanner.scan(&context, &reporter);

    let counts = scan.report().counts();

    eprintln!();

    eprintln!("diagprint scan: root={}", scan.root().display());

    eprintln!(
        "diagprint scan: profile={} files={} analyzers={} findings={}",
        scan.profile().as_str(),
        scan.files_scanned(),
        scan.analyzers_run(),
        scan.report().len(),
    );

    eprintln!(
        "diagprint scan: trace={} debug={} info={} warning={} error={} fatal={}",
        counts.trace, counts.debug, counts.info, counts.warning, counts.error, counts.fatal,
    );

    let rendered = render_report(scan.report(), scan_args.format)?;

    write_output(&rendered, scan_args.output.as_deref())?;

    if let Some(capsule_path) = scan_args.capsule.as_deref() {
        write_scan_capsule(
            scan.report(),
            scan.profile(),
            scan.files_scanned(),
            scan.analyzers_run(),
            capsule_path,
        )?;
    }

    Ok(i32::from(scan.report().exit_code()))
}

fn run_capsule_command(args: Vec<String>) -> Result<i32, Box<dyn Error>> {
    let Some(command) = args.first() else {
        print_capsule_usage();
        return Ok(2);
    };

    if matches!(command.as_str(), "-h" | "--help" | "help") {
        print_capsule_usage();
        return Ok(0);
    }

    let Some(path) = args.get(1) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("capsule {command} requires a capsule directory"),
        )
        .into());
    };

    if args.len() > 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "capsule commands accept exactly one capsule directory",
        )
        .into());
    }

    let path = Path::new(path);

    match command.as_str() {
        "verify" => {
            verify_capsule(path)?;
        }

        "inspect" => {
            inspect_capsule(path)?;
        }

        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown capsule command {other:?}; expected `verify` or `inspect`"),
            )
            .into());
        }
    }

    Ok(0)
}

fn verify_capsule(path: &Path) -> Result<(), Box<dyn Error>> {
    let verification = DiagnosticCapsule::verify_directory(path)?;

    println!("CAPSULE VERIFIED");

    println!("directory: {}", path.display(),);

    println!("report: {}", verification.report_digest,);

    println!("entries: {}", verification.entries,);

    println!("manifest: {}", verification.manifest_digest,);

    Ok(())
}

fn inspect_capsule(path: &Path) -> Result<(), Box<dyn Error>> {
    let verification = DiagnosticCapsule::verify_directory(path)?;

    let manifest_path = path.join("manifest.json");

    let manifest_bytes = fs::read(&manifest_path)?;

    let manifest: DiagnosticCapsuleManifest = serde_json::from_slice(&manifest_bytes)?;

    println!("DIAGPRINT CAPSULE");

    println!("directory: {}", path.display(),);

    println!("schema: {}", manifest.schema,);

    println!("report: {}", manifest.report_digest,);

    println!("manifest: {}", verification.manifest_digest,);

    println!("entries: {}", manifest.entries.len(),);

    println!("sources-included: {}", manifest.policy.include_sources,);

    println!(
        "remediation-plan-included: {}",
        manifest.policy.include_remediation_plan,
    );

    println!();

    for entry in manifest.entries {
        println!("{}", entry.path,);

        println!("  kind: {:?}", entry.kind,);

        println!("  media-type: {}", entry.media_type,);

        println!("  bytes: {}", entry.byte_length,);

        println!("  digest: {}", entry.artifact_digest,);
    }

    Ok(())
}

fn parse_scan_args(args: Vec<String>) -> Result<ScanArgs, Box<dyn Error>> {
    let mut path = None;

    let mut profile = ProjectScanProfile::Standard;

    let mut format = OutputFormat::Compiler;

    let mut output = None;
    let mut capsule = None;

    let mut index = 0usize;

    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => {
                print_scan_usage();
                process::exit(0);
            }

            "--static" => {
                profile = ProjectScanProfile::Static;
            }

            "--deep" => {
                profile = ProjectScanProfile::Deep;
            }

            "--profile" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--profile"));
                };

                profile = parse_profile(value)?;
            }

            "--format" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--format"));
                };

                format = parse_format(value)?;
            }

            "-o" | "--output" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--output"));
                };

                output = Some(PathBuf::from(value));
            }

            "--capsule" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--capsule"));
                };

                capsule = Some(PathBuf::from(value));
            }

            value if value.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown scan option {value:?}"),
                )
                .into());
            }

            value => {
                if path.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "scan accepts only one project path",
                    )
                    .into());
                }

                path = Some(PathBuf::from(value));
            }
        }

        index += 1;
    }

    Ok(ScanArgs {
        path: path.unwrap_or_else(|| PathBuf::from(".")),

        profile,
        format,
        output,
        capsule,
    })
}

fn parse_profile(value: &str) -> Result<ProjectScanProfile, Box<dyn Error>> {
    match value {
        "static" | "quick" => Ok(ProjectScanProfile::Static),

        "standard" => Ok(ProjectScanProfile::Standard),

        "deep" => Ok(ProjectScanProfile::Deep),

        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown scan profile {value:?}; expected static, standard, or deep"),
        )
        .into()),
    }
}

fn parse_format(value: &str) -> Result<OutputFormat, Box<dyn Error>> {
    match value {
        "compiler" | "editor" => {
            Ok(
                OutputFormat::Compiler,
            )
        }

        "plain" | "text" => {
            Ok(
                OutputFormat::Plain,
            )
        }

        "audit" => {
            Ok(
                OutputFormat::Audit,
            )
        }

        "markdown" | "md" => {
            Ok(
                OutputFormat::Markdown,
            )
        }

        "html" => {
            Ok(
                OutputFormat::Html,
            )
        }

        _ => Err(
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unknown output format {value:?}; expected compiler, plain, audit, markdown, or html"
                ),
            )
            .into(),
        ),
    }
}

fn collect_external_evidence(context: &mut ProjectContext) {
    let profile = context.profile();

    let root = context.root().to_path_buf();

    if profile == ProjectScanProfile::Static {
        return;
    }

    if root.join("Cargo.toml").is_file() {
        context.insert_tool_output(
            ProjectTool::CargoMetadata,
            capture_command(&root, "cargo", &["metadata", "--format-version=1"]),
        );
    }

    if root.join(".git").exists() {
        context.insert_tool_output(
            ProjectTool::GitStatus,
            capture_command(&root, "git", &["status", "--porcelain=v1", "--branch"]),
        );
    }

    if profile != ProjectScanProfile::Deep {
        return;
    }

    if root.join("Cargo.toml").is_file() {
        context.insert_tool_output(
            ProjectTool::CargoCheck,
            capture_command(
                &root,
                "cargo",
                &[
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--message-format=json",
                ],
            ),
        );

        context.insert_tool_output(
            ProjectTool::Clippy,
            capture_command(
                &root,
                "cargo",
                &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--message-format=json",
                ],
            ),
        );

        context.insert_tool_output(
            ProjectTool::Tests,
            capture_command(
                &root,
                "cargo",
                &[
                    "test",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--no-fail-fast",
                ],
            ),
        );
    }
}

fn capture_command(root: &Path, program: &str, args: &[&str]) -> ProjectToolOutput {
    let command = if args.is_empty() {
        program.to_owned()
    } else {
        format!("{program} {}", args.join(" "),)
    };

    match Command::new(program).args(args).current_dir(root).output() {
        Ok(output) => ProjectToolOutput::new(
            command,
            output.status.success(),
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ),

        Err(error) => {
            ProjectToolOutput::new(command, false, None, String::new(), error.to_string())
        }
    }
}

fn render_report(
    report: &DiagnosticReport,
    format: OutputFormat,
) -> Result<String, Box<dyn Error>> {
    match format {
        OutputFormat::Compiler => Ok(ReportRenderer::render_report(
            &CompilerTextRenderer,
            report.iter(),
        )),

        OutputFormat::Plain => Ok(ReportRenderer::render_report(&PlainRenderer, report.iter())),

        OutputFormat::Audit => Ok(AuditTranscriptRenderer.render_report(report)?),

        OutputFormat::Markdown => Ok(ReportRenderer::render_report(
            &MarkdownRenderer,
            report.iter(),
        )),

        OutputFormat::Html => render_html(report),
    }
}

fn write_scan_capsule(
    report: &DiagnosticReport,
    profile: ProjectScanProfile,
    files_scanned: usize,
    analyzers_run: usize,
    destination: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut capsule = DiagnosticCapsule::new(report)?;

    let compiler = CompilerTextRenderer.render_report_artifact(report)?;

    let plain = PlainRenderer.render_report_artifact(report)?;

    let markdown = MarkdownRenderer.render_report_artifact(report)?;

    let audit = AuditTranscriptRenderer.render_report_artifact(report)?;

    capsule
        .add_rendered(&compiler)?
        .add_rendered(&plain)?
        .add_rendered(&markdown)?
        .add_rendered(&audit)?;

    add_html_to_capsule(&mut capsule, report)?;

    let provenance = CapsuleProvenance::new()
        .attribute("scan_profile", profile.as_str())
        .attribute("files_scanned", files_scanned.to_string())
        .attribute("analyzers_run", analyzers_run.to_string())
        .attribute("findings", report.len().to_string())
        .attribute(
            "report_status",
            if report.has_errors() {
                "failure"
            } else {
                "success"
            },
        );

    capsule.add_provenance(&provenance)?;

    let persisted = capsule.write_to(destination)?;

    eprintln!(
        "diagprint scan: wrote capsule {}",
        persisted.directory().display(),
    );

    eprintln!(
        "diagprint scan: capsule entries={} manifest={}",
        persisted.entries(),
        persisted.manifest_digest(),
    );

    Ok(())
}

#[cfg(feature = "html")]
fn add_html_to_capsule(
    capsule: &mut DiagnosticCapsule,
    report: &DiagnosticReport,
) -> Result<(), Box<dyn Error>> {
    let html = HtmlRenderer::new()
        .with_title("diagprint project scan")
        .render_report_artifact(report)?;

    capsule.add_rendered(&html)?;

    Ok(())
}

#[cfg(not(feature = "html"))]
fn add_html_to_capsule(
    _capsule: &mut DiagnosticCapsule,
    _report: &DiagnosticReport,
) -> Result<(), Box<dyn Error>> {
    Ok(())
}

#[cfg(feature = "html")]
fn render_html(report: &DiagnosticReport) -> Result<String, Box<dyn Error>> {
    Ok(HtmlRenderer::new()
        .with_title("diagprint project scan")
        .render_report(report.iter()))
}

#[cfg(not(feature = "html"))]
fn render_html(_report: &DiagnosticReport) -> Result<String, Box<dyn Error>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "HTML output requires the `html` feature",
    )
    .into())
}

fn write_output(rendered: &str, output: Option<&Path>) -> Result<(), Box<dyn Error>> {
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, rendered)?;

        eprintln!("diagprint scan: wrote {}", path.display());

        return Ok(());
    }

    let stdout = io::stdout();

    let mut stdout = stdout.lock();

    stdout.write_all(rendered.as_bytes())?;

    if !rendered.ends_with('\n') {
        stdout.write_all(b"\n")?;
    }

    stdout.flush()?;

    Ok(())
}

fn missing_value(option: &str) -> Box<dyn Error> {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{option} requires a value"),
    )
    .into()
}

fn print_usage() {
    println!(
        "diagprint\n\
         \n\
         USAGE:\n\
           diagprint scan [PATH] [OPTIONS]\n\
           diagprint capsule <COMMAND> <CAPSULE>\n\
         \n\
         COMMANDS:\n\
           scan       Scan a project and produce diagnostics\n\
           capsule    Verify or inspect a diagnostic capsule\n\
         \n\
         Run `diagprint scan --help` or `diagprint capsule --help` for details."
    );
}

fn print_scan_usage() {
    println!(
        "diagprint scan\n\
         \n\
         USAGE:\n\
           diagprint scan [PATH] [OPTIONS]\n\
         \n\
         PROFILES:\n\
           static      Filesystem/static analysis only; executes nothing\n\
           standard    Static analysis + Cargo metadata + Git status (default)\n\
           deep        Adds cargo check, Clippy, and tests; may execute project code\n\
         \n\
         OPTIONS:\n\
           --profile <PROFILE>\n\
           --static\n\
           --deep\n\
           --format <compiler|plain|audit|markdown|html>\n\
           -o, --output <FILE>\n\
           --capsule <DIR>\n\
           -h, --help\n\
         \n\
         CAPSULE:\n\
           --capsule writes a verified diagprint capsule directory containing\n\
           JSON, compiler text, plain text, Markdown, audit transcript,\n\
           rendered receipts, provenance, and HTML when enabled.\n\
         \n\
         EXAMPLES:\n\
           diagprint scan .\n\
           diagprint scan . --static\n\
           diagprint scan . --deep\n\
           diagprint scan . --deep --format audit -o reports/project.audit\n\
           diagprint scan . --deep --capsule reports/project.diagpack"
    );
}

fn print_capsule_usage() {
    println!(
        "diagprint capsule\n\
         \n\
         USAGE:\n\
           diagprint capsule verify <CAPSULE>\n\
           diagprint capsule inspect <CAPSULE>\n\
         \n\
         COMMANDS:\n\
           verify     Verify every manifest-listed payload\n\
           inspect    Verify first, then display capsule contents\n\
         \n\
         EXAMPLES:\n\
           diagprint capsule verify reports/project.diagpack\n\
           diagprint capsule inspect reports/project.diagpack"
    );
}
