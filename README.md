# diagprint

Pretty, structured diagnostics and reports for Rust applications.

[![CI](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml/badge.svg)](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)

> **Current version:** `0.3.1`

`diagprint` is a structured diagnostic and reporting library for Rust applications and command-line tools.

It combines rich terminal diagnostics with machine-readable output, source-code context, error chains, log rotation, compression, customizable themes, and per-session diagnostic metadata.

## Features

- Rich boxed terminal diagnostics
- Unicode-aware display width
- Automatic terminal-width detection
- Line wrapping instead of blind truncation
- Source snippets with line numbers and caret highlighting
- Intelligent source clipping around the highlighted range
- Target-line source gutter markers
- Wrapped multiline source labels
- Hierarchical error causes
- Automatic `std::error::Error::source()` chain capture
- Severity levels:
  - `TRACE`
  - `DEBUG`
  - `INFO`
  - `WARNING`
  - `ERROR`
  - `FATAL`
- JSON output
- Markdown output
- Plain-text output
- Severity filtering
- File-based diagnostic reporting
- Size-based log rotation
- Hourly and daily rotation
- Rotated-file retention
- Optional gzip and Zstandard compression
- Thread-safe file writes
- UUIDv7 session IDs
- UUIDv7 report IDs
- Application, timestamp, PID, and hostname metadata
- Custom terminal themes
- True-color RGB styles
- ANSI-256 styles
- Foreground and background colors
- Bold, dim, italic, and underline modifiers
- Optional Cybercore theme integration
- Renderer regression and golden-output tests

## Quick Start

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

## Source Diagnostics

Attach source locations directly to a diagnostic:

```rust
let diagnostic = reporter
    .error("Invalid configuration value")
    .code("CFG-001")
    .label(
        "src/config.rs",
        42,
        Some(18),
        Some(8),
        Some("invalid configuration value"),
    );
```

When the source file is available, `diagprint` displays surrounding context and follows the highlighted region even when it occurs far into a long source line.

Example:

```text
│ --> src/config.rs:42:18                                      │
│   41 │ let mode = read_mode();                               │
│ > 42 │ …configuration.set_mode(invalid_mode);                │
│      │                   ^^^^^^^^                             │
│      │                   └─ invalid configuration value      │
│   43 │ start_runtime();                                      │
```

## Error Chains

Existing Rust error chains can be captured through `std::error::Error`:

```rust
let diagnostic = reporter
    .error("Operation failed")
    .code("APP-001")
    .from_error(&error);

reporter.emit(&diagnostic)?;
```

`Error::source()` is followed automatically.

## Output Formats

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

The structured diagnostic model remains independent from the renderer.

## Themes

`diagprint` includes a customizable terminal theme system.

```rust
use diagprint::{Reporter, Style, Theme};

let theme = Theme {
    border: Style::rgb(0, 255, 255).dim(),
    source_caret: Style::rgb(255, 0, 128).bold(),
    note: Style::rgb(255, 180, 0).bold(),
    help: Style::rgb(120, 255, 120).bold(),
    ..Theme::default()
};

let reporter = Reporter::builder()
    .application("myapp")
    .theme(theme)
    .build()?;
```

### Style API

True-color foreground:

```rust
Style::rgb(20, 185, 181)
```

True-color background:

```rust
Style::rgb(255, 255, 255)
    .on_rgb(14, 9, 29)
```

Hex colors:

```rust
Style::from_hex("#14B9B5")
```

ANSI-256:

```rust
Style::ansi256(51)
```

Modifiers:

```rust
Style::rgb(253, 62, 106)
    .bold()
    .underline()
```

Available modifiers:

- `.bold()`
- `.dim()`
- `.italic()`
- `.underline()`

Setting:

```rust
.color(false)
```

on `ReporterBuilder` suppresses all ANSI styling regardless of theme.

## Cybercore Integration

Cybercore integration is optional.

Enable it with:

```bash
cargo run --features cybercore --example cybercore
```

Use the active Cybercore theme:

```rust
use diagprint::Theme;

let theme = Theme::cybercore();
```

Use a named theme:

```rust
let theme = Theme::cybercore_named("neon-night")
    .expect("Cybercore theme exists");
```

Safely fall back to the active theme:

```rust
let theme = Theme::cybercore_or_default("possibly-missing-theme");
```

List available Cybercore themes:

```rust
let themes = Theme::cybercore_theme_names();
```

Check whether a theme exists:

```rust
if Theme::cybercore_theme_exists("neon-night") {
    println!("theme available");
}
```

Read the active theme name:

```rust
let active = Theme::cybercore_active_theme_name();
```

The Cybercore schema remains the source of truth. `diagprint` does not maintain a duplicate palette registry.

### Cybercore Showcase

List embedded Cybercore themes:

```bash
cargo run --features cybercore --example cybercore -- --list
```

Render the active theme:

```bash
cargo run --features cybercore --example cybercore
```

Render a specific theme:

```bash
cargo run --features cybercore --example cybercore -- neon-night
```

Use Cybercore's environment override:

```bash
CYBERGRID_THEME=neon-night \
cargo run --features cybercore --example cybercore
```

The showcase renders every severity from `TRACE` through `FATAL`, making it useful as a quick visual palette audit.

## Log Rotation

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

Available cadences:

- `RotationCadence::Never`
- `RotationCadence::Hourly`
- `RotationCadence::Daily`

## Compression

Compression support is optional.

Build with:

```bash
cargo build --features compression
```

Configure it with:

```rust
use diagprint::Compression;

let reporter = Reporter::builder()
    .application("myapp")
    .file("reports/myapp.log")
    .compression(Compression::Zstd)
    .build()?;
```

Available modes:

```rust
Compression::None
Compression::Gzip
Compression::Zstd
```

## Feature Flags

### `compression`

Enables:

- gzip
- Zstandard

```bash
cargo test --features compression
```

### `cybercore`

Enables Cybercore schema and theme integration.

```bash
cargo test --features cybercore
```

### All Features

```bash
cargo test --all-features
```

## Development

Format:

```bash
cargo fmt --all -- --check
```

Check:

```bash
cargo check --all-targets
```

Clippy:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Tests:

```bash
cargo test --all-targets --all-features
```

Run the standard example:

```bash
cargo run --example basic
```

Run the error-chain example:

```bash
cargo run --example error_chain
```

Run the Cybercore showcase:

```bash
cargo run --features cybercore --example cybercore
```

## Project Status

`diagprint` is under active development.

Version `0.3.1` focuses on terminal rendering quality and theme infrastructure.

The API may continue to evolve before `1.0`.

## Roadmap

### v0.4 — Integrations

- Dedicated `anyhow` integration
- `thiserror` integration tests and examples
- `tracing` integration
- Improved `std::error::Error` adapters
- Custom diagnostic metadata

### Future

- Nonblocking output
- Async writer backend
- HTML renderer
- Custom renderer registration
- Improved rotation collision handling
- Compressed archive retention awareness
- Machine-readable diagnostic protocol

## Repository

https://github.com/darkstardevx/diagprint

Bug reports, feature requests, and contributions are welcome.

## License

`diagprint` is dual-licensed under either:

- Apache License, Version 2.0
- MIT License

at your option.

See `LICENSE-APACHE` and `LICENSE-MIT`.
