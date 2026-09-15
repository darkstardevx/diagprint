# diagprint

[![CI](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml/badge.svg)](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml)
[![GitHub Release](https://img.shields.io/github/v/release/darkstardevx/diagprint)](https://github.com/darkstardevx/diagprint/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Pretty, structured diagnostics and reports for Rust applications.

`diagprint` turns application failures into structured diagnostics that can be
rendered for terminals, files, JSON, Markdown, or plain text.

v0.4 adds **Diagnostic Intelligence**: structured suggestions, documentation
links, guarded text edits, dry-run validation, interactive fixes, and
documentation that can be viewed directly in the terminal with syntax
highlighting.

## Highlights

- Rich boxed terminal diagnostics
- Unicode-aware display widths
- Automatic terminal sizing
- Source snippets with highlighted ranges
- Intelligent source clipping around the actual error
- Hierarchical error chains
- Notes and help text
- JSON, Markdown, plain-text, and terminal renderers
- UUIDv7 session and report identifiers
- File rotation and retention
- Optional gzip and Zstandard compression
- Custom terminal themes
- Optional Cybercore theme integration
- Structured diagnostic suggestions
- Documentation links
- Machine-applicable text edits
- Stale-edit protection
- UTF-8 boundary validation
- Overlapping-edit rejection
- Dry-run fix validation
- Optional backups
- Interactive fix workflow
- Optional terminal documentation viewer
- Syntax-highlighted documentation examples
- Remote terminal-control sanitization
- Remote document size limits

## Installation

Until the crate is published to a registry, use the GitHub repository.

```toml
[dependencies]
diagprint = {
    git = "https://github.com/darkstardevx/diagprint.git",
    tag = "v0.4.0"
}
```

Enable optional features as needed:

```toml
[dependencies]
diagprint = {
    git = "https://github.com/darkstardevx/diagprint.git",
    tag = "v0.4.0",
    features = ["compression", "terminal-docs"]
}
```

Cybercore integration is available separately:

```toml
[dependencies]
diagprint = {
    git = "https://github.com/darkstardevx/diagprint.git",
    tag = "v0.4.0",
    features = ["cybercore"]
}
```

## Quick Start

```rust
use diagprint::{Reporter, Severity};

fn main() -> diagprint::Result<()> {
    let reporter = Reporter::builder()
        .application("myapp")
        .min_severity(Severity::Info)
        .build()?;

    let diagnostic = reporter
        .error("Network initialization failed")
        .code("NET-001")
        .cause("failed to open network interface")
        .note("Fallback networking is unavailable")
        .help("Check interface permissions and driver state");

    reporter.emit(&diagnostic)?;

    Ok(())
}
```

## Source Diagnostics

Diagnostics can point directly at source locations.

```rust
let diagnostic = reporter
    .error("Invalid configuration value")
    .code("CFG-001")
    .label(
        "config.toml",
        12,
        Some(9),
        Some(5),
        Some("unsupported value"),
    );
```

The terminal renderer follows the highlighted range rather than blindly
truncating long source lines from the beginning.

## Error Chains

`diagprint` supports hierarchical causes:

```rust
let diagnostic = reporter
    .error("Request failed")
    .cause("connection failed")
    .cause("DNS lookup failed");
```

Existing `std::error::Error` chains can also be captured:

```rust
let diagnostic = reporter
    .error("Operation failed")
    .from_error(&error);
```

## Diagnostic Intelligence

v0.4 introduces structured suggestions.

```rust
use diagprint::{
    Applicability, DocumentationLink, Edit, Suggestion, TextRange,
};

let original =
    r#"chrono = { version = "0.4", features = ["clock"] }"#;

let replacement =
    r#"chrono = { version = "0.4", features = ["clock", "serde"] }"#;

let diagnostic = reporter
    .error("chrono::DateTime cannot be serialized")
    .code("CARGO-001")
    .suggestion(
        Suggestion::new("Enable chrono's serde feature")
            .explanation(
                "chrono only provides serde implementations when its \
                 `serde` feature is enabled",
            )
            .applicability(Applicability::MachineApplicable)
            .documentation(
                DocumentationLink::docs_rs(
                    "chrono",
                    "latest",
                    "",
                ),
            )
            .edit(Edit::replace(
                "Cargo.toml",
                TextRange::new(0, original.len()),
                original,
                replacement,
            )),
    );
```

Terminal output includes the proposed change:

```text
SUGGESTION
TITLE  Enable chrono's serde feature
WHY    chrono only provides serde implementations when its `serde`
       feature is enabled

PATCH  Cargo.toml
- chrono = { version = "0.4", features = ["clock"] }
+ chrono = { version = "0.4", features = ["clock", "serde"] }

DOCS   chrono documentation
       https://docs.rs/chrono/latest/chrono/

APPLY  machine-applicable
FIX    automatic fix available
```

## Applicability

Every suggestion has an applicability level:

```rust
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Manual,
}
```

Only `MachineApplicable` suggestions containing structured edits are eligible
for automatic application.

The classification alone is not enough. `diagprint` validates the current
filesystem again before writing.

## Validate Without Changing Files

Use `Fixer::check()` as a dry-run:

```rust
use diagprint::Fixer;

let fixer = Fixer::new();

let check = fixer.check(&diagnostic)?;

println!(
    "{} applicable suggestion(s)",
    check.applicable_suggestions
);

for file in check.affected_files {
    println!("would change: {}", file.display());
}
```

No files are modified.

## Apply Fixes

```rust
let report = Fixer::new()
    .backups(true)
    .apply(&diagnostic)?;

println!(
    "{} suggestion(s) applied",
    report.applied_suggestions
);
```

Before changing a file, `diagprint` verifies:

- the edit is machine-applicable;
- the byte range is valid;
- edit offsets are UTF-8 boundaries;
- edits do not overlap;
- the current file still contains the exact expected text.

If the source changed after the diagnostic was generated, the edit is rejected
instead of guessing.

## Interactive Fixes

```rust
Fixer::new()
    .backups(true)
    .apply_interactive(&diagnostic)?;
```

The terminal workflow exposes only actions that are actually available:

```text
FIX  Enable chrono's serde feature
APPLICABILITY  machine-applicable
VERIFY  current file contents match the proposed edit
[A]pply  [S]kip  [D]ocs  [Q]uit >
```

If validation fails, `[A]pply` disappears:

```text
VERIFY  blocked: refusing stale edit in Cargo.toml ...
[S]kip  [D]ocs  [Q]uit >
```

## Suggested Commands

Suggestions may include follow-up commands:

```rust
use diagprint::SuggestedCommand;

let suggestion = Suggestion::new("Enable serde")
    .command(
        SuggestedCommand::new("cargo check")
            .explanation(
                "Verify the project after applying the edit",
            ),
    );
```

Commands are **never executed automatically**.

They are advisory information only.

## Documentation Links

Documentation links are structured data:

```rust
let rust = DocumentationLink::rust_error("E0277");

let cargo = DocumentationLink::cargo_book(
    "reference/features.html",
);

let chrono = DocumentationLink::docs_rs(
    "chrono",
    "latest",
    "",
);
```

Custom links are also supported:

```rust
let link = DocumentationLink::new(
    "Project troubleshooting guide",
    "https://example.com/docs/troubleshooting",
);
```

## Terminal Documentation

Enable:

```toml
diagprint = {
    git = "https://github.com/darkstardevx/diagprint.git",
    tag = "v0.4.0",
    features = ["terminal-docs"]
}
```

Then:

```rust
use diagprint::{
    DocumentationLink,
    TerminalDocViewer,
};

let link = DocumentationLink::rust_error("E0277");

TerminalDocViewer::new()
    .width(96)
    .open_and_print(&link)?;
```

The viewer:

- accepts HTTP and HTTPS documentation URLs;
- retrieves the document synchronously;
- limits remote document size;
- sanitizes terminal control characters;
- converts HTML to readable terminal text;
- extracts code examples;
- syntax-highlights code with 24-bit ANSI output.

The default maximum remote document size is 2 MiB.

Because the current viewer uses a blocking HTTP client, applications already
inside an async runtime should call it from an appropriate blocking worker.

### Demo

```bash
cargo run --features terminal-docs --example terminal_docs
```

Or:

```bash
cargo run \
    --features terminal-docs \
    --example terminal_docs -- \
    https://docs.rs/chrono/latest/chrono/
```

## Intelligence Demo

Preview:

```bash
cargo run --example intelligence
```

Validate without writing:

```bash
cargo run --example intelligence -- --check
```

Apply:

```bash
cargo run --example intelligence -- --apply
```

Interactive mode with terminal docs:

```bash
cargo run \
    --features terminal-docs \
    --example intelligence \
    -- --interactive
```

The demo operates on:

```text
target/diagprint-demo/Cargo.toml
```

rather than your project's real manifest.

## Themes

Terminal presentation is controlled by:

```rust
use diagprint::{SeverityTheme, Style, Theme};
```

Create a custom theme:

```rust
let theme = Theme {
    border: Style::rgb(20, 185, 181),
    patch_add: Style::rgb(100, 255, 100),
    patch_remove: Style::rgb(255, 80, 100),
    ..Theme::default()
};
```

Styles support:

- standard ANSI foregrounds;
- ANSI-256 foregrounds and backgrounds;
- 24-bit RGB foregrounds and backgrounds;
- hex colors;
- bold;
- dim;
- italic;
- underline.

`color(false)` remains authoritative and disables ANSI styling regardless of
the selected theme.

## Cybercore Integration

Enable:

```toml
diagprint = {
    git = "https://github.com/darkstardevx/diagprint.git",
    tag = "v0.4.0",
    features = ["cybercore"]
}
```

Use the active Cybercore theme:

```rust
use diagprint::Theme;

let theme = Theme::cybercore();
```

Or select a named theme:

```rust
let theme = Theme::cybercore_or_default("neon-night");
```

Useful helpers:

```rust
Theme::cybercore_theme_names();
Theme::cybercore_active_theme_name();
Theme::cybercore_theme_exists("neon-night");
Theme::cybercore_named("neon-night");
```

`diagprint` consumes Cybercore's semantic palette rather than duplicating its
hex values.

### Showcase

```bash
cargo run \
    --features cybercore \
    --example cybercore
```

List Cybercore themes:

```bash
cargo run \
    --features cybercore \
    --example cybercore -- \
    --list
```

Choose one:

```bash
cargo run \
    --features cybercore \
    --example cybercore -- \
    neon-night
```

## Output Formats

The same diagnostic data can be rendered as:

- terminal output;
- plain text;
- JSON;
- Markdown.

This keeps diagnostic construction independent from presentation.

## Rotation

File reports support:

- size-based rotation;
- hourly rotation;
- daily rotation;
- retention cleanup.

```rust
use diagprint::{RotationCadence, RotationPolicy};

let policy = RotationPolicy {
    cadence: RotationCadence::Daily,
    ..Default::default()
};
```

## Compression

Enable:

```toml
features = ["compression"]
```

Supported formats:

```rust
use diagprint::Compression;

// Compression::Gzip
// Compression::Zstd
```

## Feature Flags

| Feature | Purpose |
| --- | --- |
| `compression` | gzip and Zstandard report compression |
| `cybercore` | Cybercore theme-schema integration |
| `terminal-docs` | documentation retrieval and terminal syntax highlighting |

All optional features are disabled by default.

## Safety Model

`diagprint` intentionally separates presentation from mutation.

Rendering a diagnostic does not modify files.

`Fixer` only applies structured text edits that:

1. are marked `MachineApplicable`;
2. still match the expected source content;
3. use valid UTF-8 boundaries;
4. do not overlap.

Suggested shell commands are never automatically executed.

Terminal documentation also sanitizes remote control characters before
display and limits the amount of remote content accepted.

## Development

Default feature gate:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
```

Full feature gate:

```bash
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo test --doc --all-features
```

Examples:

```bash
cargo run --example basic
cargo run --example error_chain
cargo run --example intelligence

cargo run \
    --features cybercore \
    --example cybercore

cargo run \
    --features terminal-docs \
    --example terminal_docs
```

## Project Status

### v0.4.0 — Diagnostic Intelligence

v0.4 expands `diagprint` from a diagnostic presentation engine into a
structured diagnostic assistance system.

The guiding rule remains:

> A diagnostic may explain and propose. Mutation must be explicit, structured,
> validated, and reject uncertainty.

## Roadmap

Potential future work includes:

- `anyhow` integration;
- expanded `thiserror` integration;
- `tracing` integration;
- asynchronous/nonblocking report output;
- async terminal documentation retrieval;
- richer unified diff rendering;
- additional renderers;
- additional structured fix sources.

## Registry Publication

Registry publication is intentionally disabled in v0.4 release preparation
while the optional Cybercore dependency is sourced from GitHub.

GitHub releases remain fully supported.

Once Cybercore is available from the target registry, the publishing guard can
be removed and the registry package verified independently.

## Repository

https://github.com/darkstardevx/diagprint

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
