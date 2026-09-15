# diagprint

Pretty, structured diagnostics and reports for Rust applications.

[![CI](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml/badge.svg)](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

> **Current version:** `0.3.0`

`diagprint` is a structured diagnostic and reporting library for Rust applications and command-line tools.

It combines rich terminal diagnostics with machine-readable output, source-code context, error chains, log rotation, compression, and per-session diagnostic metadata.

## Features

* Width-aware boxed terminal diagnostics
* Severity levels: `TRACE`, `DEBUG`, `INFO`, `WARNING`, `ERROR`, and `FATAL`
* ANSI-colored terminal output
* Source snippets with line numbers and caret labels
* Hierarchical error causes
* Automatic `std::error::Error::source()` chain capture
* JSON output
* Markdown output
* Plain-text output
* Severity filtering
* File-based diagnostic reporting
* Size-based log rotation
* Hourly and daily rotation
* Rotated-file retention
* Optional gzip and Zstandard compression
* Thread-safe file writes through a shared mutex
* UUIDv7 session IDs
* UUIDv7 report IDs
* Automatic timestamps
* Application name, PID, and hostname metadata
* Diagnostic codes, notes, and help messages

## Example

```rust
use diagprint::{Reporter, RotationCadence, Severity};

fn main() -> diagprint::Result<()> {
    let reporter = Reporter::builder()
        .application("myapp")
        .min_severity(Severity::Info)
        .file("reports/myapp.log")
        .max_file_size(10 * 1024 * 1024)
        .rotation_count(5)
        .rotation_cadence(RotationCadence::Daily)
        .show_metadata(true)
        .width(82)
        .build()?;

    let diagnostic = reporter
        .error("Network initialization failed")
        .code("NET-001")
        .cause("failed to open network interface")
        .cause("permission denied")
        .note("Fallback interface was not selected")
        .help("Check interface permissions and driver state");

    reporter.emit(&diagnostic)?;

    Ok(())
}
```

Example terminal output:

```text
╭─ ✖ ERROR [NET-001] ──────────────────────────────────────────────╮
│ Network initialization failed                                   │
│                                                                  │
│ Caused by                                                        │
│ └─ failed to open network interface                              │
│    └─ permission denied                                          │
│                                                                  │
│ NOTE  Fallback interface was not selected                        │
│ HELP  Check interface permissions and driver state               │
├─ Diagnostic ─────────────────────────────────────────────────────┤
│ App       myapp                                                  │
│ PID       12345                                                  │
│ Host      workstation                                            │
│ Session   0199...                                                │
│ Report    0199...                                                │
╰──────────────────────────────────────────────────────────────────╯
```

## Source Diagnostics

Diagnostics can include source locations and highlighted ranges:

```rust
let diagnostic = reporter
    .error("Invalid configuration value")
    .code("CFG-001")
    .label(
        "config.rs",
        42,
        Some(12),
        Some(8),
        Some("invalid value"),
    )
    .help("Use one of the supported configuration values");
```

When the source file is available, `diagprint` displays surrounding source lines and highlights the relevant location.

## Rust Error Chains

`diagprint` can capture an existing `std::error::Error` chain:

```rust
let diagnostic = reporter
    .error("Operation failed")
    .code("APP-001")
    .from_error(&error);

reporter.emit(&diagnostic)?;
```

`Error::source()` is followed automatically and represented as a hierarchical diagnostic cause chain.

## Output Formats

The same structured `Diagnostic` can be rendered in multiple formats.

### Terminal

```rust
reporter.emit(&diagnostic)?;
```

### JSON

```rust
reporter.emit_json(&diagnostic)?;
```

### Markdown

```rust
reporter.emit_markdown(&diagnostic)?;
```

The renderer architecture keeps diagnostic data separate from its presentation.

## Log Rotation

`diagprint` supports size-based and time-based rotation.

```rust
use diagprint::RotationCadence;

let reporter = Reporter::builder()
    .application("myapp")
    .file("reports/myapp.log")
    .max_file_size(10 * 1024 * 1024)
    .rotation_count(10)
    .rotation_cadence(RotationCadence::Daily)
    .build()?;
```

Available rotation cadences:

* `RotationCadence::Never`
* `RotationCadence::Hourly`
* `RotationCadence::Daily`

Old rotated reports are removed according to the
