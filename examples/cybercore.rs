use diagprint::{Reporter, Severity, Theme};
use std::env;

fn print_usage(program: &str) {
    println!("Cybercore theme showcase for diagprint");
    println!();
    println!("Usage:");
    println!("  {program}");
    println!("  {program} <theme>");
    println!("  {program} --list");
    println!();
    println!("Examples:");
    println!("  {program} neon-night");
    println!("  CYBERGRID_THEME=neon-night {program}");
}

fn print_themes() {
    let active = Theme::cybercore_active_theme_name();
    let names = Theme::cybercore_theme_names();

    println!("Available Cybercore themes:");
    println!();

    for name in names {
        if name == active {
            println!("  * {name}  [active]");
        } else {
            println!("    {name}");
        }
    }
}

fn main() -> diagprint::Result<()> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "cybercore".into());
    let requested = args.next();

    if matches!(requested.as_deref(), Some("-h" | "--help")) {
        print_usage(&program);
        return Ok(());
    }

    if matches!(requested.as_deref(), Some("-l" | "--list")) {
        print_themes();
        return Ok(());
    }

    let active = Theme::cybercore_active_theme_name();

    let (theme_name, theme) = match requested {
        Some(name) if Theme::cybercore_theme_exists(&name) => {
            let theme = Theme::cybercore_or_default(&name);
            (name, theme)
        }

        Some(name) => {
            eprintln!(
                "diagprint: Cybercore theme `{name}` was not found; \
                 falling back to active theme `{active}`"
            );
            eprintln!();

            (active.clone(), Theme::cybercore_or_default(&name))
        }

        None => (active.clone(), Theme::cybercore()),
    };

    println!("diagprint // Cybercore theme showcase");
    println!("theme: {theme_name}");
    println!();

    let reporter = Reporter::builder()
        .application("diagprint-cybercore")
        .theme(theme)
        .min_severity(Severity::Trace)
        .width(76)
        .build()?;

    let diagnostics = [
        reporter
            .trace("Telemetry packet entered the diagnostic pipeline")
            .code("TRACE-001")
            .note("Low-level execution details are available"),
        reporter
            .debug("CYBERGRID resolver completed configuration discovery")
            .code("DEBUG-001")
            .note("Resolved the embedded Cybercore schema"),
        reporter
            .info("Diagnostic uplink established successfully")
            .code("INFO-001")
            .help("No action is required"),
        reporter
            .warning("Fallback relay is operating with reduced capacity")
            .code("WARN-001")
            .cause("primary relay did not answer before timeout")
            .help("Inspect relay health before the next deployment"),
        reporter
            .error("CYBERGRID diagnostic uplink failed")
            .code("ERROR-001")
            .cause("node handshake rejected")
            .cause("authentication signature mismatch")
            .note("Fallback relay remained offline")
            .help("Verify the node identity and reconnect to CYBERGRID"),
        reporter
            .fatal("Diagnostic core entered an unrecoverable state")
            .code("FATAL-001")
            .cause("persistent state could not be recovered")
            .note("Normal execution cannot continue")
            .help("Preserve the report and terminate the process"),
    ];

    for diagnostic in diagnostics {
        reporter.emit(&diagnostic)?;
    }

    Ok(())
}
