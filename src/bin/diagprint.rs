use diagprint::{
    CapsuleProvenance, DIAGNOSTIC_HISTORY_RUN_V2_SCHEMA, DiagnosticCapsule,
    DiagnosticCapsuleManifest, DiagnosticHistory, DiagnosticReport, DiagnosticTimeline,
    DiagnosticTimelineEvent, DiagnosticTimelinePhase, DiagnosticTimelineRun, GitProvenanceBinding,
    GitProvenanceRecord, Reporter,
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
    history: Option<PathBuf>,
    history_label: Option<String>,
    git_provenance: bool,
}

#[derive(Debug)]
struct RecordedHistory {
    run_index: usize,
    run_count: usize,
    current_report: String,
    previous_report: Option<String>,
    run_digest: String,
    chain_head: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitSnapshot {
    commit: String,
    tree: String,
    parents: Vec<String>,
}

#[derive(Debug)]
struct BlameArgs {
    history: PathBuf,
    fingerprint: String,
    run: Option<usize>,
    repository: Option<PathBuf>,
}

#[derive(Debug)]
struct GitCommitInfo {
    author: String,
    authored_at: String,
    subject: String,
}

#[derive(Debug)]
struct GitDiffSummary {
    base: String,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
    binary_files: usize,
    changed: Vec<String>,
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

        "history" => run_history_command(args.collect()),

        "why" => run_why_command(args.collect()),



        "timeline" => run_timeline_command(args.collect()),




        "blame" => run_blame_command(args.collect()),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command {other:?}; expected `scan`, `capsule`, `history`, `why`, `timeline`, or `blame`"),
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

    let git_snapshot = if scan_args.git_provenance {
        Some(capture_clean_git_snapshot(context.root())?)
    } else {
        None
    };

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

    if let Some(snapshot) = &git_snapshot {
        verify_clean_git_snapshot(context.root(), snapshot)?;
    }

    let recorded_history = match scan_args.history.as_deref() {
        Some(history_directory) => Some(record_scan_history(
            scan.report(),
            history_directory,
            scan_args.history_label.as_deref(),
        )?),

        None => None,
    };

    let recorded_git_provenance = match (
        recorded_history.as_ref(),
        git_snapshot.as_ref(),
        scan_args.history.as_deref(),
    ) {
        (Some(history), Some(snapshot), Some(history_directory)) => Some(
            record_captured_git_provenance(history_directory, history, snapshot)?,
        ),

        _ => None,
    };

    if let Some(capsule_path) = scan_args.capsule.as_deref() {
        write_scan_capsule(
            scan.report(),
            scan.profile(),
            scan.files_scanned(),
            scan.analyzers_run(),
            recorded_history.as_ref(),
            recorded_git_provenance.as_ref(),
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
        "verify" => verify_capsule(path)?,

        "inspect" => inspect_capsule(path)?,

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

fn run_blame_command(args: Vec<String>) -> Result<i32, Box<dyn Error>> {
    if args.len() == 1 && matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_blame_usage();
        return Ok(0);
    }

    if args.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: diagprint blame <HISTORY> <FINGERPRINT> [--run <N>] [--repo <PATH>]",
        )
        .into());
    }

    let mut run = None;
    let mut repository = None;

    let mut index = 2usize;

    while index < args.len() {
        match args[index].as_str() {
            "--run" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--run"));
                };

                run = Some(value.parse::<usize>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid history run index {value:?}"),
                    )
                })?);
            }

            "--repo" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--repo"));
                };

                repository = Some(PathBuf::from(value));
            }

            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown blame option {other:?}"),
                )
                .into());
            }
        }

        index += 1;
    }

    show_git_blame(&BlameArgs {
        history: PathBuf::from(&args[0]),
        fingerprint: args[1].clone(),
        run,
        repository,
    })?;

    Ok(0)
}

fn run_timeline_command(args: Vec<String>) -> Result<i32, Box<dyn Error>> {
    if args.len() == 1 && matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_timeline_usage();
        return Ok(0);
    }

    if args.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: diagprint timeline <HISTORY> <FINGERPRINT>",
        )
        .into());
    }

    show_history_timeline(Path::new(&args[0]), &args[1])?;

    Ok(0)
}

fn run_why_command(args: Vec<String>) -> Result<i32, Box<dyn Error>> {
    if args.len() == 1 && matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_why_usage();
        return Ok(0);
    }

    if args.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: diagprint why <HISTORY> <FINGERPRINT>",
        )
        .into());
    }

    show_history_why(Path::new(&args[0]), &args[1])?;

    Ok(0)
}

fn run_history_command(args: Vec<String>) -> Result<i32, Box<dyn Error>> {
    let Some(command) = args.first() else {
        print_history_usage();
        return Ok(2);
    };

    if matches!(command.as_str(), "-h" | "--help" | "help") {
        print_history_usage();
        return Ok(0);
    }

    match command.as_str() {
        "verify" => {
            if args.len() != 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: diagprint history verify <HISTORY>",
                )
                .into());
            }

            verify_history(Path::new(&args[1]))?;
        }

        "show" => {
            if args.len() != 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: diagprint history show <HISTORY>",
                )
                .into());
            }

            show_history(Path::new(&args[1]))?;
        }

        "fingerprints" | "findings" => {
            if args.len() != 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: diagprint history fingerprints <HISTORY>",
                )
                .into());
            }

            show_history_fingerprints(Path::new(&args[1]))?;
        }

        "git-bind" => {
            run_history_git_bind(&args[1..])?;
        }

        "timeline" => {
            if args.len() != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: diagprint history timeline <HISTORY> <FINGERPRINT>",
                )
                .into());
            }

            show_history_timeline(Path::new(&args[1]), &args[2])?;
        }

        "why" => {
            if args.len() != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: diagprint history why <HISTORY> <FINGERPRINT>",
                )
                .into());
            }

            show_history_why(Path::new(&args[1]), &args[2])?;
        }

        "lineage" => {
            if args.len() != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "usage: diagprint history lineage <HISTORY> <FINGERPRINT>",
                )
                .into());
            }

            show_history_lineage(Path::new(&args[1]), &args[2])?;
        }

        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unknown history command {other:?}; expected `verify`, `show`, `fingerprints`, `lineage`, `why`, `timeline`, or `git-bind`"
                ),
            )
            .into());
        }
    }

    Ok(0)
}

fn verify_capsule(path: &Path) -> Result<(), Box<dyn Error>> {
    let verification = DiagnosticCapsule::verify_directory(path)?;

    println!("CAPSULE VERIFIED");
    println!("directory: {}", path.display());
    println!("report: {}", verification.report_digest);
    println!("entries: {}", verification.entries);
    println!("manifest: {}", verification.manifest_digest);

    Ok(())
}

fn inspect_capsule(path: &Path) -> Result<(), Box<dyn Error>> {
    let verification = DiagnosticCapsule::verify_directory(path)?;

    let manifest_path = path.join("manifest.json");

    let manifest_bytes = fs::read(&manifest_path)?;

    let manifest: DiagnosticCapsuleManifest = serde_json::from_slice(&manifest_bytes)?;

    println!("DIAGPRINT CAPSULE");
    println!("directory: {}", path.display());
    println!("schema: {}", manifest.schema);
    println!("report: {}", manifest.report_digest);
    println!("manifest: {}", verification.manifest_digest);
    println!("entries: {}", manifest.entries.len());
    println!("sources-included: {}", manifest.policy.include_sources);

    println!(
        "remediation-plan-included: {}",
        manifest.policy.include_remediation_plan,
    );

    println!();

    for entry in manifest.entries {
        println!("{}", entry.path);
        println!("  kind: {:?}", entry.kind);
        println!("  media-type: {}", entry.media_type);
        println!("  bytes: {}", entry.byte_length);
        println!("  digest: {}", entry.artifact_digest);
    }

    Ok(())
}

fn verify_history(path: &Path) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(path)?;

    history.verify()?;

    println!("HISTORY VERIFIED");
    println!("directory: {}", path.display());
    println!("schema: {DIAGNOSTIC_HISTORY_RUN_V2_SCHEMA}");
    println!("runs: {}", history.len());
    println!("fingerprints: {}", history.fingerprints().len());

    match history.head_digest() {
        Some(head) => {
            println!("chain-head: {head}");
        }

        None => {
            println!("chain-head: none");
        }
    }

    if let Some(latest) = history.latest() {
        println!("latest-run: {:06}", latest.index);
        println!("latest-report: {}", latest.report_digest);
    }

    Ok(())
}

fn record_scan_history(
    report: &DiagnosticReport,
    directory: &Path,
    requested_label: Option<&str>,
) -> Result<RecordedHistory, Box<dyn Error>> {
    let mut history = DiagnosticHistory::open(directory)?;

    let previous_report = history.latest().map(|run| run.report_digest.clone());

    let next_index = history.len();

    let label = requested_label
        .map(str::to_owned)
        .unwrap_or_else(|| format!("scan-{next_index:06}"));

    let (run_index, current_report, run_digest) = {
        let run = history.append_report(label.clone(), report)?;

        (
            run.index,
            run.report_digest.clone(),
            run.run_digest.to_string(),
        )
    };

    let chain_head = history
        .head_digest()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "history append succeeded but produced no chain head",
            )
        })?
        .to_string();

    let run_count = history.len();

    eprintln!("diagprint history: recorded run={run_index:06} label={label:?}",);

    eprintln!("diagprint history: report={current_report}");
    eprintln!("diagprint history: chain-head={chain_head}");

    if let Some(transition) = history.latest_transition()? {
        eprintln!(
            "diagprint history: new={} resolved={} persisting={} changed={} errors+={} severity+={} reappeared={}",
            transition.counts.new,
            transition.counts.resolved,
            transition.counts.persisting,
            transition.counts.changed,
            transition.introduced_errors,
            transition.severity_increases,
            latest_reappearances(&history),
        );
    } else {
        eprintln!("diagprint history: baseline established");
    }

    Ok(RecordedHistory {
        run_index,
        run_count,
        current_report,
        previous_report,
        run_digest,
        chain_head,
    })
}

fn latest_reappearances(history: &DiagnosticHistory) -> usize {
    history
        .fingerprints()
        .into_iter()
        .filter(|fingerprint| {
            let lineage = history.lineage(fingerprint);

            let Some(latest) = lineage.steps.last() else {
                return false;
            };

            let Some(previous) = lineage.steps.iter().rev().nth(1) else {
                return false;
            };

            latest.active_instances != 0
                && previous.active_instances == 0
                && lineage
                    .first_seen_run()
                    .is_some_and(|first_seen| first_seen < latest.to_run)
        })
        .count()
}

fn open_existing_history(path: &Path) -> Result<DiagnosticHistory, Box<dyn Error>> {
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "diagnostic history directory does not exist: {}",
                path.display()
            ),
        )
        .into());
    }

    Ok(DiagnosticHistory::open(path)?)
}

fn show_history(path: &Path) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(path)?;

    println!("DIAGPRINT HISTORY");
    println!("directory: {}", path.display());
    println!("schema: {DIAGNOSTIC_HISTORY_RUN_V2_SCHEMA}");
    println!("runs: {}", history.len());
    println!("fingerprints: {}", history.fingerprints().len());

    match history.head_digest() {
        Some(head) => {
            println!("chain-head: {head}");
        }

        None => {
            println!("chain-head: none");
        }
    }

    if history.is_empty() {
        println!();
        println!("no history runs recorded");
        return Ok(());
    }

    println!();

    for run in history.runs() {
        println!(
            "RUN {:06} label={:?} diagnostics={} failures={}",
            run.index,
            run.label,
            run.diagnostics,
            run.severity.failures(),
        );

        println!("  report: {}", run.report_digest);
        println!("  run-digest: {}", run.run_digest);

        match run.previous_run_digest {
            Some(previous) => {
                println!("  previous-run: {previous}");
            }

            None => {
                println!("  previous-run: none");
            }
        }

        println!(
            "  severity: trace={} debug={} info={} warning={} error={} fatal={}",
            run.severity.trace,
            run.severity.debug,
            run.severity.info,
            run.severity.warning,
            run.severity.error,
            run.severity.fatal,
        );

        if run.index != 0 {
            let transition = history.transition(run.index - 1, run.index)?;

            println!(
                "  delta: new={} resolved={} persisting={} changed={} errors+={} severity+={}",
                transition.counts.new,
                transition.counts.resolved,
                transition.counts.persisting,
                transition.counts.changed,
                transition.introduced_errors,
                transition.severity_increases,
            );
        }
    }

    Ok(())
}

fn show_history_fingerprints(path: &Path) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(path)?;

    let fingerprints = history.fingerprints();

    println!("DIAGPRINT HISTORY FINGERPRINTS");
    println!("directory: {}", path.display());
    println!("fingerprints: {}", fingerprints.len());

    if fingerprints.is_empty() {
        println!();
        println!("no diagnostic fingerprints recorded");
        return Ok(());
    }

    println!();

    for fingerprint in fingerprints {
        let lineage = history.lineage(&fingerprint);

        println!(
            "{}  active={} first={:?} last={:?} appearances={} changes={} resolutions={} reappeared={}",
            short_fingerprint(&fingerprint),
            lineage.active_in_latest(),
            lineage.first_seen_run(),
            lineage.last_seen_run(),
            lineage.appearances(),
            lineage.changes(),
            lineage.resolutions(),
            lineage.reappeared_after_absence(),
        );

        println!("  {fingerprint}");
    }

    Ok(())
}

fn show_history_timeline(path: &Path, query: &str) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(path)?;

    // Match `why`: presentation is permitted only after re-verifying the
    // persisted chain immediately before constructing forensic evidence.
    history.verify()?;

    let fingerprint = resolve_fingerprint(&history, query)?;

    let timeline = history.timeline(&fingerprint).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no forensic timeline exists for diagnostic fingerprint {fingerprint:?}"),
        )
    })?;

    println!("DIAGNOSTIC TIMELINE");
    println!("schema: {}", timeline.schema);
    println!("directory: {}", path.display());
    println!("fingerprint: {}", timeline.fingerprint);
    println!("status: {}", timeline.status.as_str());
    println!("chain-verified: true");

    match &timeline.chain_head {
        Some(head) => {
            println!("chain-head: {head}");
        }

        None => {
            println!("chain-head: none");
        }
    }

    println!("history-runs: {}", timeline.history_runs);
    println!("active-runs: {}", timeline.active_runs());
    println!("absent-runs: {}", timeline.absent_runs());
    println!("unseen-runs: {}", timeline.unseen_runs());
    println!("first-seen: {:06}", timeline.first_seen_run);
    println!("last-seen: {:06}", timeline.last_seen_run);
    println!("active-instances: {}", timeline.active_instances);
    println!("reappearances: {}", timeline.reappearances);

    println!();

    let track = timeline
        .runs
        .iter()
        .map(timeline_marker)
        .collect::<Vec<_>>()
        .join(" ");

    println!("TRACK  {track}");
    println!(
        "       ● first/active   ▲ severity+   ◆ changed   \
○ resolved   ↻ reappeared   · absent/unseen"
    );

    println!();
    println!("RUNS");

    for run in &timeline.runs {
        let marker = timeline_marker(run);

        let episode = run
            .episode
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_owned());

        let severities = if run.severities.is_empty() {
            "-".to_owned()
        } else {
            run.severities
                .iter()
                .map(|(severity, count)| format!("{severity}={count}"))
                .collect::<Vec<_>>()
                .join(" ")
        };

        let events = if run.events.is_empty() {
            "-".to_owned()
        } else {
            run.events
                .iter()
                .map(|event| event.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };

        println!(
            "  {marker} {:06} {:7} ep={} n={} severity=[{}] events=[{}] label={:?}",
            run.run_index,
            run.phase.as_str(),
            episode,
            run.instances,
            severities,
            events,
            run.label,
        );

        if run.introduced_instances != 0
            || run.resolved_instances != 0
            || run.persisting_instances != 0
            || run.changed_instances != 0
            || run.severity_increases != 0
        {
            println!(
                "      delta: new={} resolved={} persisting={} changed={} severity+={}",
                run.introduced_instances,
                run.resolved_instances,
                run.persisting_instances,
                run.changed_instances,
                run.severity_increases,
            );
        }

        for digest in &run.diagnostic_digests {
            println!("      digest: {digest}");
        }
    }

    println!();
    println!("CLEAN WINDOWS");

    if timeline.clean_windows.is_empty() {
        println!("  none");
    } else {
        for window in &timeline.clean_windows {
            println!(
                "  {:06}..{:06} runs={} kind={}",
                window.start_run,
                window.end_run,
                window.runs,
                window.kind.as_str(),
            );
        }
    }

    Ok(())
}

fn timeline_marker(run: &DiagnosticTimelineRun) -> &'static str {
    if run.events.contains(&DiagnosticTimelineEvent::Reappeared) {
        return "↻";
    }

    if run
        .events
        .contains(&DiagnosticTimelineEvent::SeverityIncreased)
    {
        return "▲";
    }

    if run.events.contains(&DiagnosticTimelineEvent::Changed) {
        return "◆";
    }

    if run.events.contains(&DiagnosticTimelineEvent::Resolved) {
        return "○";
    }

    if run.events.contains(&DiagnosticTimelineEvent::FirstSeen) {
        return "●";
    }

    match run.phase {
        DiagnosticTimelinePhase::Active => "●",
        DiagnosticTimelinePhase::Unseen | DiagnosticTimelinePhase::Absent => "·",
    }
}

fn show_history_why(path: &Path, query: &str) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(path)?;

    // Re-verify immediately before constructing forensic evidence so a
    // post-open mutation cannot be silently presented as trustworthy.
    history.verify()?;

    let fingerprint = resolve_fingerprint(&history, query)?;

    let case = history.case_file(&fingerprint).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no forensic evidence exists for diagnostic fingerprint {fingerprint:?}"),
        )
    })?;

    let first = case.evidence.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "forensic case file has no first evidence run",
        )
    })?;

    let last = case.evidence.last().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "forensic case file has no last evidence run",
        )
    })?;

    println!("DIAGNOSTIC CASE FILE");
    println!("schema: {}", case.schema);
    println!("directory: {}", path.display());
    println!("fingerprint: {}", case.fingerprint);
    println!("status: {}", case.status.as_str());
    println!("chain-verified: true");

    match &case.chain_head {
        Some(head) => {
            println!("chain-head: {head}");
        }

        None => {
            println!("chain-head: none");
        }
    }

    println!(
        "first-seen: run={:06} label={:?}",
        first.run_index, first.label,
    );

    println!(
        "last-seen: run={:06} label={:?}",
        last.run_index, last.label,
    );

    println!("history-runs: {}", case.history_runs,);

    println!("observed-runs: {}", case.observed_runs,);

    println!("observed-instances: {}", case.observed_instances,);

    println!("active-instances: {}", case.active_instances,);

    println!("unique-digests: {}", case.unique_digests,);

    println!("episodes: {}", case.episodes.len(),);

    println!("reappearances: {}", case.reappearances,);

    println!("introduced-instances: {}", case.introduced_instances,);

    println!("resolved-instances: {}", case.resolved_instances,);

    println!("changed-instances: {}", case.changed_instances,);

    println!("severity-increases: {}", case.severity_increases,);

    println!();
    println!("EPISODES");

    for episode in &case.episodes {
        match episode.resolved_run {
            Some(resolved_run) => {
                println!(
                    "  #{:02} start={:06} last-active={:06} resolved={:06} runs={} instances={} peak={}",
                    episode.episode,
                    episode.started_run,
                    episode.last_active_run,
                    resolved_run,
                    episode.observed_runs,
                    episode.instances,
                    episode.peak_instances,
                );
            }

            None => {
                println!(
                    "  #{:02} start={:06} last-active={:06} resolved=active runs={} instances={} peak={}",
                    episode.episode,
                    episode.started_run,
                    episode.last_active_run,
                    episode.observed_runs,
                    episode.instances,
                    episode.peak_instances,
                );
            }
        }
    }

    println!();
    println!("EVIDENCE");

    for evidence in &case.evidence {
        let severities = evidence
            .severities
            .iter()
            .map(|(severity, count)| format!("{severity}={count}"))
            .collect::<Vec<_>>()
            .join(" ");

        println!(
            "  RUN {:06} label={:?} instances={} severity=[{}]",
            evidence.run_index, evidence.label, evidence.instances, severities,
        );

        println!("    report: {}", evidence.report_digest,);

        println!("    run-digest: {}", evidence.run_digest,);

        println!(
            "    diagnostic-digests: {}",
            evidence.diagnostic_digests.len(),
        );

        for digest in &evidence.diagnostic_digests {
            println!("      {digest}");
        }
    }

    Ok(())
}

fn show_history_lineage(path: &Path, query: &str) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(path)?;

    let fingerprint = resolve_fingerprint(&history, query)?;

    let lineage = history.lineage(&fingerprint);

    println!("DIAGPRINT LINEAGE");
    println!("directory: {}", path.display());
    println!("fingerprint: {}", lineage.fingerprint);
    println!("first-seen: {:?}", lineage.first_seen_run());
    println!("last-seen: {:?}", lineage.last_seen_run());
    println!("active: {}", lineage.active_in_latest());
    println!("reappeared: {}", lineage.reappeared_after_absence());
    println!("appearances: {}", lineage.appearances());
    println!("changes: {}", lineage.changes());
    println!("resolutions: {}", lineage.resolutions());
    println!();

    for step in lineage.steps {
        match step.from_run {
            Some(from_run) => {
                println!(
                    "RUN {from_run:06} -> {:06}  new={} resolved={} persisting={} changed={} severity+={} active={}",
                    step.to_run,
                    step.counts.new,
                    step.counts.resolved,
                    step.counts.persisting,
                    step.counts.changed,
                    step.severity_increases,
                    step.active_instances,
                );
            }

            None => {
                println!(
                    "RUN {:06}  new={} active={}",
                    step.to_run, step.counts.new, step.active_instances,
                );
            }
        }
    }

    Ok(())
}

fn resolve_fingerprint(history: &DiagnosticHistory, query: &str) -> Result<String, Box<dyn Error>> {
    if query.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "diagnostic fingerprint query must not be empty",
        )
        .into());
    }

    let fingerprints = history.fingerprints();

    if let Some(exact) = fingerprints
        .iter()
        .find(|fingerprint| fingerprint.as_str() == query)
    {
        return Ok(exact.clone());
    }

    let matches = fingerprints
        .iter()
        .filter(|fingerprint| fingerprint_matches(fingerprint, query))
        .cloned()
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [single] => Ok(single.clone()),

        [] => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no diagnostic fingerprint matches {query:?}"),
        )
        .into()),

        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "diagnostic fingerprint prefix {query:?} is ambiguous across {} fingerprints",
                matches.len(),
            ),
        )
        .into()),
    }
}

fn fingerprint_matches(fingerprint: &str, query: &str) -> bool {
    fingerprint.starts_with(query)
        || fingerprint
            .rsplit(':')
            .next()
            .is_some_and(|hex| hex.starts_with(query))
}

fn short_fingerprint(fingerprint: &str) -> &str {
    let hex = fingerprint.rsplit(':').next().unwrap_or(fingerprint);

    let length = hex.len().min(12);

    &hex[..length]
}

fn run_history_git_bind(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: diagprint history git-bind <HISTORY> <RUN> <COMMIT> [--repo <PATH>]",
        )
        .into());
    }

    let history_path = Path::new(&args[0]);

    let run_index = args[1].parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid history run index {:?}", args[1]),
        )
    })?;

    let revision = &args[2];

    let mut repository = None;

    let mut index = 3usize;

    while index < args.len() {
        match args[index].as_str() {
            "--repo" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--repo"));
                };

                repository = Some(PathBuf::from(value));
            }

            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown git-bind option {other:?}"),
                )
                .into());
            }
        }

        index += 1;
    }

    let history = open_existing_history(history_path)?;
    history.verify()?;

    let run = history.runs().get(run_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "history run {run_index} is out of range for {} run(s)",
                history.len(),
            ),
        )
    })?;

    let repository = resolve_repository(repository.as_deref(), history_path)?;

    let snapshot = resolve_git_snapshot(&repository, revision)?;

    let record =
        GitProvenanceRecord::user_asserted(run, snapshot.commit, snapshot.tree, snapshot.parents)?;

    let path = record.persist(&history)?;

    println!("GIT PROVENANCE BOUND");
    println!("history: {}", history_path.display());
    println!("run: {:06}", run.index);
    println!("run-digest: {}", run.run_digest);
    println!("binding: {}", record.binding.as_str());
    println!("commit: {}", record.commit);
    println!("tree: {}", record.tree);
    println!("record-digest: {}", record.record_digest);
    println!("record: {}", path.display());
    println!("causation: NOT ESTABLISHED");

    Ok(())
}

fn show_git_blame(args: &BlameArgs) -> Result<(), Box<dyn Error>> {
    let history = open_existing_history(&args.history)?;

    history.verify()?;

    let fingerprint = resolve_fingerprint(&history, &args.fingerprint)?;

    let timeline = history.timeline(&fingerprint).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no forensic timeline exists for diagnostic fingerprint {fingerprint:?}"),
        )
    })?;

    let (run_index, record) = select_blame_record(&history, &timeline, args.run)?;

    let run = timeline.runs.get(run_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("timeline does not contain history run {run_index}"),
        )
    })?;

    let repository = resolve_repository(args.repository.as_deref(), &args.history)?;

    let resolved = resolve_git_snapshot(&repository, &record.commit)?;

    if resolved.commit != record.commit
        || resolved.tree != record.tree
        || resolved.parents != record.parents
    {
        return Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                "recorded Git commit no longer resolves to the recorded tree and parent identities in this repository",
            )
            .into(),
        );
    }

    let commit = query_git_commit_info(&repository, &record.commit)?;

    let diff = git_diff_summary(&repository, &record)?;

    let events = if run.events.is_empty() {
        "none".to_owned()
    } else {
        run.events
            .iter()
            .map(|event| event.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };

    println!("DIAGNOSTIC GIT PROVENANCE");
    println!("schema: {}", record.schema);
    println!("history: {}", args.history.display());
    println!("fingerprint: {fingerprint}");
    println!("history-run: {:06}", run_index);
    println!("history-label: {:?}", run.label);
    println!("history-run-digest: {}", record.history_run_digest);
    println!("report-digest: {}", record.report_digest);
    println!("binding: {}", record.binding.as_str());
    println!("provenance-record: {}", record.record_digest);
    println!("history-binding-verified: true");
    println!("git-object-verified: true");

    println!();
    println!("TRANSITION");
    println!("  phase: {}", run.phase.as_str());
    println!("  events: {events}");
    println!("  instances: {}", run.instances);
    println!("  changed: {}", run.changed_instances);
    println!("  severity-increases: {}", run.severity_increases);

    println!();
    println!("COMMIT");
    println!("  commit: {}", record.commit);
    println!("  tree: {}", record.tree);

    if record.parents.is_empty() {
        println!("  parents: none");
    } else {
        println!("  parents: {}", record.parents.join(" "));
    }

    println!("  author: {}", safe_terminal_text(&commit.author,),);

    println!(
        "  authored-at: {}",
        safe_terminal_text(&commit.authored_at,),
    );

    println!("  subject: {}", safe_terminal_text(&commit.subject,),);

    println!();
    println!("REPOSITORY DIFF");
    println!("  base: {}", diff.base);
    println!("  files-changed: {}", diff.files_changed);
    println!("  insertions: {}", diff.insertions);
    println!("  deletions: {}", diff.deletions);
    println!("  binary-files: {}", diff.binary_files);

    if diff.changed.is_empty() {
        println!("  changed-files: none");
    } else {
        println!("  changed-files:");

        for entry in diff.changed.iter().take(20) {
            println!("    {}", safe_terminal_text(entry,),);
        }

        if diff.changed.len() > 20 {
            println!("    ... {} additional path(s)", diff.changed.len() - 20,);
        }
    }

    println!();
    println!("ASSESSMENT");

    match record.binding {
        GitProvenanceBinding::CapturedClean => {
            println!(
                "  provenance: commit/tree captured from a clean worktree surrounding this scan"
            );
        }

        GitProvenanceBinding::UserAsserted => {
            println!(
                "  provenance: commit/run association was explicitly asserted after the history run"
            );
        }
    }

    println!(
        "  association: repository change context is temporally associated with this history run"
    );

    println!("  causation: NOT ESTABLISHED");

    println!(
        "  note: changed files are repository context only; diagprint does not claim they caused this diagnostic transition"
    );

    Ok(())
}

fn select_blame_record(
    history: &DiagnosticHistory,
    timeline: &DiagnosticTimeline,
    requested: Option<usize>,
) -> Result<(usize, GitProvenanceRecord), Box<dyn Error>> {
    if let Some(run_index) = requested {
        if run_index >= history.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "history run {run_index} is out of range for {} run(s)",
                    history.len(),
                ),
            )
            .into());
        }

        let record = GitProvenanceRecord::load(history, run_index)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("history run {run_index:06} has no Git provenance binding"),
            )
        })?;

        return Ok((run_index, record));
    }

    for run in timeline.runs.iter().rev().filter(|run| {
        run.events
            .iter()
            .any(|event| !matches!(event, DiagnosticTimelineEvent::Persisting))
    }) {
        if let Some(record) = GitProvenanceRecord::load(history, run.run_index)? {
            return Ok((run.run_index, record));
        }
    }

    for run in timeline.runs.iter().rev() {
        if let Some(record) = GitProvenanceRecord::load(history, run.run_index)? {
            return Ok((run.run_index, record));
        }
    }

    Err(
        io::Error::new(
            io::ErrorKind::NotFound,
            "no Git provenance binding exists for any retained run of this diagnostic; capture one with `scan --git-provenance` or backfill one with `history git-bind`",
        )
        .into(),
    )
}

fn record_captured_git_provenance(
    history_directory: &Path,
    recorded: &RecordedHistory,
    snapshot: &GitSnapshot,
) -> Result<GitProvenanceRecord, Box<dyn Error>> {
    let history = DiagnosticHistory::open(history_directory)?;

    let run = history.runs().get(recorded.run_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "recorded history run {} disappeared before Git provenance persistence",
                recorded.run_index,
            ),
        )
    })?;

    if run.run_digest.to_string() != recorded.run_digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "recorded history run digest changed before Git provenance persistence",
        )
        .into());
    }

    let record = GitProvenanceRecord::captured_clean(
        run,
        snapshot.commit.clone(),
        snapshot.tree.clone(),
        snapshot.parents.clone(),
    )?;

    let path = record.persist(&history)?;

    eprintln!(
        "diagprint git provenance: run={:06} binding={} commit={}",
        run.index,
        record.binding.as_str(),
        record.commit,
    );

    eprintln!(
        "diagprint git provenance: record={} path={}",
        record.record_digest,
        path.display(),
    );

    Ok(record)
}

fn capture_clean_git_snapshot(root: &Path) -> Result<GitSnapshot, Box<dyn Error>> {
    let status = git_output(root, &["status", "--porcelain=v1", "--untracked-files=all"])?;

    if !status.trim().is_empty() {
        return Err(
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "--git-provenance requires a clean Git worktree before scanning; `git status --porcelain` returned:\n{}",
                    status.trim_end(),
                ),
            )
            .into(),
        );
    }

    resolve_git_snapshot(root, "HEAD")
}

fn verify_clean_git_snapshot(root: &Path, expected: &GitSnapshot) -> Result<(), Box<dyn Error>> {
    let actual = capture_clean_git_snapshot(root)?;

    if &actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Git HEAD/tree changed while the diagnostic scan was running",
        )
        .into());
    }

    Ok(())
}

fn resolve_git_snapshot(repository: &Path, revision: &str) -> Result<GitSnapshot, Box<dyn Error>> {
    let commit_spec = format!("{revision}^{{commit}}");

    let commit = git_output(repository, &["rev-parse", "--verify", &commit_spec])?
        .trim()
        .to_owned();

    let tree_spec = format!("{commit}^{{tree}}");

    let tree = git_output(repository, &["rev-parse", "--verify", &tree_spec])?
        .trim()
        .to_owned();

    let parents = git_output(repository, &["show", "-s", "--format=%P", &commit])?
        .split_whitespace()
        .map(str::to_owned)
        .collect();

    Ok(GitSnapshot {
        commit,
        tree,
        parents,
    })
}

fn resolve_repository(explicit: Option<&Path>, history: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let probe = explicit.unwrap_or(history);

    let result = git_output(probe, &["rev-parse", "--show-toplevel"]);

    let root = match result {
        Ok(root) => root,

        Err(error) => {
            if explicit.is_none() {
                return Err(
                        io::Error::new(
                            io::ErrorKind::NotFound,
                            format!(
                                "could not discover a Git repository from history path {}; use --repo <PATH>: {error}",
                                history.display(),
                            ),
                        )
                        .into(),
                    );
            }

            return Err(error);
        }
    };

    Ok(PathBuf::from(root.trim()))
}

fn query_git_commit_info(repository: &Path, commit: &str) -> Result<GitCommitInfo, Box<dyn Error>> {
    let author = git_output(repository, &["show", "-s", "--format=%an", commit])?
        .trim_end()
        .to_owned();

    let authored_at = git_output(repository, &["show", "-s", "--format=%aI", commit])?
        .trim_end()
        .to_owned();

    let subject = git_output(repository, &["show", "-s", "--format=%s", commit])?
        .trim_end()
        .to_owned();

    Ok(GitCommitInfo {
        author,
        authored_at,
        subject,
    })
}

fn git_diff_summary(
    repository: &Path,
    record: &GitProvenanceRecord,
) -> Result<GitDiffSummary, Box<dyn Error>> {
    let (base, numstat, name_status) = match record.parents.first() {
        Some(parent) => (
            parent.clone(),
            git_output(
                repository,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--numstat",
                    "--find-renames",
                    parent.as_str(),
                    record.commit.as_str(),
                ],
            )?,
            git_output(
                repository,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--name-status",
                    "--find-renames",
                    parent.as_str(),
                    record.commit.as_str(),
                ],
            )?,
        ),

        None => (
            "<root>".to_owned(),
            git_output(
                repository,
                &[
                    "diff-tree",
                    "--root",
                    "--no-commit-id",
                    "-r",
                    "--numstat",
                    record.commit.as_str(),
                ],
            )?,
            git_output(
                repository,
                &[
                    "diff-tree",
                    "--root",
                    "--no-commit-id",
                    "-r",
                    "--name-status",
                    record.commit.as_str(),
                ],
            )?,
        ),
    };

    let (insertions, deletions, binary_files) = parse_numstat(&numstat);

    let changed = name_status
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();

    Ok(GitDiffSummary {
        base,

        files_changed: changed.len(),

        insertions,

        deletions,

        binary_files,

        changed,
    })
}

fn parse_numstat(output: &str) -> (usize, usize, usize) {
    let mut insertions = 0usize;
    let mut deletions = 0usize;
    let mut binary_files = 0usize;

    for line in output.lines() {
        let mut fields = line.splitn(3, '\t');

        let added = fields.next().unwrap_or_default();

        let removed = fields.next().unwrap_or_default();

        match (added.parse::<usize>(), removed.parse::<usize>()) {
            (Ok(added), Ok(removed)) => {
                insertions = insertions.saturating_add(added);

                deletions = deletions.saturating_add(removed);
            }

            _ => {
                binary_files = binary_files.saturating_add(1);
            }
        }
    }

    (insertions, deletions, binary_files)
}

fn git_output(repository: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {} failed in {}: {}",
            args.join(" "),
            repository.display(),
            String::from_utf8_lossy(&output.stderr,).trim(),
        ))
        .into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn safe_terminal_text(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '�'
            } else {
                character
            }
        })
        .collect()
}

fn parse_scan_args(args: Vec<String>) -> Result<ScanArgs, Box<dyn Error>> {
    let mut path = None;

    let mut profile = ProjectScanProfile::Standard;

    let mut format = OutputFormat::Compiler;

    let mut output = None;
    let mut capsule = None;
    let mut history = None;
    let mut history_label = None;
    let mut git_provenance = false;

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

            "--history" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--history"));
                };

                history = Some(PathBuf::from(value));
            }

            "--history-label" => {
                index += 1;

                let Some(value) = args.get(index) else {
                    return Err(missing_value("--history-label"));
                };

                history_label = Some(value.clone());
            }

            "--git-provenance" => {
                git_provenance = true;
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

    if history_label.is_some() && history.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--history-label requires --history <DIR>",
        )
        .into());
    }

    if git_provenance && history.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--git-provenance requires --history <DIR>",
        )
        .into());
    }

    Ok(ScanArgs {
        path: path.unwrap_or_else(|| PathBuf::from(".")),
        profile,
        format,
        output,
        capsule,
        history,
        history_label,
        git_provenance,
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
        "compiler" | "editor" => Ok(OutputFormat::Compiler),

        "plain" | "text" => Ok(OutputFormat::Plain),

        "audit" => Ok(OutputFormat::Audit),

        "markdown" | "md" => Ok(OutputFormat::Markdown),

        "html" => Ok(OutputFormat::Html),

        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unknown output format {value:?}; expected compiler, plain, audit, markdown, or html"
            ),
        )
        .into()),
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
        format!("{program} {}", args.join(" "))
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
    history: Option<&RecordedHistory>,
    git_provenance: Option<&GitProvenanceRecord>,
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

    let mut provenance = CapsuleProvenance::new()
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

    if let Some(history) = history {
        provenance = provenance
            .attribute("history_schema", DIAGNOSTIC_HISTORY_RUN_V2_SCHEMA)
            .attribute("history_run_index", history.run_index.to_string())
            .attribute("history_run_count", history.run_count.to_string())
            .attribute("history_report", history.current_report.clone())
            .attribute("history_run_digest", history.run_digest.clone())
            .attribute("history_chain_head", history.chain_head.clone());

        if let Some(previous_report) = &history.previous_report {
            provenance = provenance.attribute("history_previous_report", previous_report.clone());
        }
    }

    if let Some(git_provenance) = git_provenance {
        provenance = provenance
            .attribute("git_provenance_schema", git_provenance.schema.clone())
            .attribute("git_provenance_binding", git_provenance.binding.as_str())
            .attribute(
                "git_provenance_record_digest",
                git_provenance.record_digest.to_string(),
            )
            .attribute("git_commit", git_provenance.commit.clone())
            .attribute("git_tree", git_provenance.tree.clone());
    }

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

    if let Some(history) = history {
        eprintln!(
            "diagprint scan: capsule anchored history run={:06} chain-head={}",
            history.run_index, history.chain_head,
        );
    }

    if let Some(git_provenance) = git_provenance {
        eprintln!(
            "diagprint scan: capsule anchored Git provenance record={}",
            git_provenance.record_digest,
        );
    }

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
           diagprint history <COMMAND> <HISTORY>\n\
           diagprint why <HISTORY> <FINGERPRINT>\n\
         \n\
         COMMANDS:\n\
           scan       Scan a project and produce diagnostics\n\
           capsule    Verify or inspect a diagnostic capsule\n\
           history    Verify and inspect persistent diagnostic history\n\
           why        Build an evidence-backed diagnostic forensic case file\n\
           timeline   Visualize one diagnostic across every retained run\n\
           blame      Bind diagnostic transitions to verified Git provenance\n\
         \n\
         Run a command with --help for details."
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
           --history <DIR>\n\
           --history-label <LABEL>\n\
           --git-provenance\n\
           -h, --help\n\
         \n\
         HISTORY:\n\
           --history appends a privacy-light, hash-chained scan state to an\n\
           append-only history directory. `.diagprint/history` is recommended.\n\
           When --capsule is also supplied, the resulting capsule provenance\n\
           anchors the exact history chain head for that scan.\n\
           --git-provenance additionally requires a clean Git worktree, binds\n\
           the new history run to the exact Git commit/tree, and stores the\n\
           binding under <HISTORY>/git-provenance/. Ignore `.diagprint/` in Git\n\
           or keep history outside the repository so generated history does not\n\
           dirty subsequent provenance-enabled scans.\n\
         \n\
         EXAMPLES:\n\
           diagprint scan .\n\
           diagprint scan . --static\n\
           diagprint scan . --deep --history .diagprint/history\n\
           diagprint scan . --deep --history .diagprint/history --history-label pre-refactor\n\
           diagprint scan . --static --history .diagprint/history --capsule reports/project.diagpack"
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
           inspect    Verify first, then display capsule contents"
    );
}

fn print_history_usage() {
    println!(
        "diagprint history\n\
         \n\
         USAGE:\n\
           diagprint history verify <HISTORY>\n\
           diagprint history show <HISTORY>\n\
           diagprint history fingerprints <HISTORY>\n\
           diagprint history lineage <HISTORY> <FINGERPRINT>\n\
           diagprint history why <HISTORY> <FINGERPRINT>\n\
         \n\
         COMMANDS:\n\
           verify         Verify run digests, chain links, sequence, and head\n\
           show           Show runs, chain identities, and semantic transitions\n\
           fingerprints   List logical finding identities and lifecycle state\n\
           lineage        Trace one logical finding across all recorded runs\n\
           why            Explain one finding as a forensic case file\n\
           timeline       Visualize lifecycle, regressions, and clean windows\n\
           git-bind        Backfill an explicit user-asserted Git/run binding\n\
         \n\
         FINGERPRINTS:\n\
           lineage, why, and timeline accept either the full canonical\n\
           fingerprint or a unique leading hexadecimal prefix shown by\n\
           `history fingerprints`."
    );
}
fn print_why_usage() {
    println!(
        "diagprint why\n\
         \n\
         USAGE:\n\
           diagprint why <HISTORY> <FINGERPRINT>\n\
         \n\
         DESCRIPTION:\n\
           Verify a diagnostic-history chain, resolve one canonical fingerprint,\n\
           and construct a privacy-light evidence-backed forensic case file.\n\
         \n\
         The fingerprint may be complete or a unique leading hexadecimal prefix.\n\
         No source-control blame or root-cause inference is performed in v1."
    );
}

fn print_timeline_usage() {
    println!(
        "diagprint timeline\n\
         \n\
         USAGE:\n\
           diagprint timeline <HISTORY> <FINGERPRINT>\n\
         \n\
         DESCRIPTION:\n\
           Verify diagnostic history and visualize one logical finding across\n\
           every retained run, including unseen periods, active episodes,\n\
           resolution, clean windows, changes, severity regressions, and\n\
           reappearances.\n\
         \n\
         GLYPHS:\n\
           ●  first observation or ordinary active run\n\
           ▲  severity increase\n\
           ◆  canonical content change\n\
           ○  resolution transition\n\
           ↻  reappearance after absence\n\
           ·  absent or not-yet-seen run\n\
         \n\
         The fingerprint may be complete or a unique leading hexadecimal prefix."
    );
}

fn print_blame_usage() {
    println!(
        "diagprint blame\n\
         \n\
         USAGE:\n\
           diagprint blame <HISTORY> <FINGERPRINT> [--run <N>] [--repo <PATH>]\n\
         \n\
         DESCRIPTION:\n\
           Verify diagnostic history, verify an immutable Git provenance binding,\n\
           verify the recorded commit/tree/parents in the local Git object store,\n\
           and display repository change context for the selected diagnostic run.\n\
         \n\
         When --run is omitted, diagprint prefers the newest bound run with a\n\
         meaningful forensic transition such as first-seen, changed, severity\n\
         increase, resolution, or reappearance.\n\
         \n\
         `blame` means provenance investigation, not causal attribution. Git\n\
         association never establishes that a commit caused the diagnostic."
    );
}
