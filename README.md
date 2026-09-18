# diagprint

[![CI](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml/badge.svg)](https://github.com/darkstardevx/diagprint/actions/workflows/ci.yml)
[![Release](https://github.com/darkstardevx/diagprint/actions/workflows/release.yml/badge.svg)](https://github.com/darkstardevx/diagprint/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/diagprint.svg)](https://crates.io/crates/diagprint)
[![Docs.rs](https://docs.rs/diagprint/badge.svg)](https://docs.rs/diagprint)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**diagprint is a Rust diagnostics lifecycle framework.**

Define diagnostics once, then carry them safely through terminal output, compiler tooling, editors, CI, code scanning, guarded remediation, verification, telemetry, and privacy-aware external reporting.

`diagprint` turns failures into actionable, structured diagnostics with:

- rich terminal rendering;
- primary and secondary source labels;
- virtual and in-memory sources;
- immutable source snapshots;
- per-source revision tracking;
- stale-source detection;
- captured revision-aware diagnostics;
- JSON, Markdown, plain-text, GitHub Actions, and SARIF output;
- ecosystem interoperability;
- rustc and Cargo diagnostic ingestion;
- guarded remediation;
- transactional multi-file fixes;
- post-fix verification;
- canonical diagnostic fingerprints and report digests;
- semantic report deltas and baseline-aware CI evaluation;
- exact-byte artifact receipts and transactional artifact persistence;
- diagnostic capsules and privacy-aware export;
- project scanning with static, standard, and deep profiles;
- hash-chained diagnostic history and lineage;
- diagnostic forensic case files and `diagprint why` analysis;
- version-aware documentation.

The core rule is:

> Diagnostics may explain and propose. Mutation must be explicit, structured,
> validated, and reject uncertainty.

## Diagnostic Forensics

Most error libraries answer:

> What went wrong right now?

diagprint can also start answering:

> What has this exact logical diagnostic been doing over time?

A scan history can be investigated with:

```bash
diagprint history fingerprints .diagprint/history
diagprint why .diagprint/history 7f23a9d5c120
```

`diagprint why` verifies the history chain and produces a privacy-light
**diagnostic case file**:

```text
DIAGNOSTIC CASE FILE
schema: diagprint.forensics.case-file/v1
fingerprint: diagprint.canonical/v1:sha256:...
status: active
chain-verified: true
first-seen: run=000002 label="baseline"
last-seen: run=000011 label="regressed"
history-runs: 12
observed-runs: 7
episodes: 2
reappearances: 1
changed-instances: 3
severity-increases: 1

EPISODES
  #01 start=000002 last-active=000006 resolved=000007 runs=5 instances=5 peak=1
  #02 start=000010 last-active=000011 resolved=active runs=2 instances=2 peak=1

EVIDENCE
  RUN 000002 label="baseline" instances=1 severity=[warning=1]
    report: diagprint.canonical/v1:sha256:...
    run-digest: sha256:...
```

The v1 case file is deliberately evidence-based. It does not guess root cause,
Git blame, or authorship.

Because diagnostic history is privacy-light, case files contain canonical
identity, lifecycle state, run labels, severity counts, and cryptographic
evidence without restoring diagnostic messages, source text, arbitrary
attributes, or remediation payloads.

This is the foundation for the larger Diagnostic Forensics roadmap:

```text
why ✓ → timeline ✓ → blame ✓ → causal graph ✓ → replay ✓
```

### Visual Timeline

The same evidence can be viewed run by run:

```bash
diagprint timeline .diagprint/history 7f23a9d5c120
```

```text
DIAGNOSTIC TIMELINE
status: active
chain-verified: true

TRACK  · ● ▲ ○ · ↻
       ● first/active   ▲ severity+   ◆ changed   ○ resolved   ↻ reappeared   · absent/unseen

RUNS
  · 000000 unseen  ep=- n=0 severity=[-] events=[-] label="prehistory"
  ● 000001 active  ep=1 n=1 severity=[warning=1] events=[first_seen] label="baseline"
  ▲ 000002 active  ep=1 n=1 severity=[error=1] events=[changed,severity_increased] label="regression"
  ○ 000003 absent  ep=- n=0 severity=[-] events=[resolved] label="fixed"
  · 000004 absent  ep=- n=0 severity=[-] events=[-] label="still-clean"
  ↻ 000005 active  ep=2 n=1 severity=[warning=1] events=[reappeared] label="regressed"

CLEAN WINDOWS
  000000..000000 runs=1 kind=before_first_seen
  000003..000004 runs=2 kind=between_episodes
```

The visual glyph is only a compact presentation. The underlying timeline keeps
the complete transition counts, severity distribution, canonical content
digests, episode identity, and run labels.

### Git Provenance and `blame`

A forensic timeline becomes much more useful when a history run can be tied to
the exact repository state that surrounded it.

Capture Git provenance while scanning:

```bash
diagprint scan . \
  --static \
  --history .diagprint/history \
  --git-provenance
```

The worktree must be clean. diagprint verifies it before and after the scan,
then binds the newly appended history run to the exact Git commit and tree.

Investigate a diagnostic transition:

```bash
diagprint blame .diagprint/history 7f23a9d5c120
```

```text
DIAGNOSTIC GIT PROVENANCE
history-run: 000012
binding: captured_clean
history-binding-verified: true
git-object-verified: true

TRANSITION
  phase: active
  events: reappeared

COMMIT
  commit: 7d19fa2...
  tree: 3c81b44...
  author: ...
  authored-at: ...
  subject: refactor config loader

REPOSITORY DIFF
  files-changed: 12
  insertions: 84
  deletions: 39
  changed-files:
    M       src/config.rs

ASSESSMENT
  provenance: commit/tree captured from a clean worktree surrounding this scan
  association: repository change context is temporally associated with this history run
  causation: NOT ESTABLISHED
```

Older histories can be backfilled explicitly:

```bash
diagprint history git-bind \
  .diagprint/history \
  12 \
  7d19fa2
```

Backfilled records are permanently marked `user_asserted`, rather than being
presented as equivalent to scan-time clean-worktree capture.

When `--git-provenance` and `--capsule` are used together, capsule provenance
also anchors the Git provenance record digest.

For repositories that keep history under `.diagprint/`, add `.diagprint/` to
`.gitignore` so diagnostic output does not dirty later provenance-enabled
scans.


### Diagnostic relationship graph

M4 adds a typed relationship graph between canonical diagnostic identities.

Relationship semantics and evidence provenance remain separate. Producer-declared
or source-chain evidence can retain explicit causal structure, while structural,
trace, temporal, and inferred evidence remain visibly classified.

Inspect a graph around one diagnostic:

```bash
diagprint graph .diagprint/history <FINGERPRINT>
```

Include inferred correlations explicitly:

```bash
diagprint graph .diagprint/history <FINGERPRINT> --evidence all
```

Export deterministic Graphviz DOT:

```bash
diagprint graph .diagprint/history <FINGERPRINT> --format dot
```

Graph traversal is cycle-safe and depth-bounded. Cascade analysis reports only
the topology of recorded explicit causal edges; it does not independently claim
root cause.

The relationship API is also the common substrate for the Ecosystem Bridges
track, allowing third-party error/diagnostic crates to preserve structured
relationships instead of flattening them into rendered strings.


## Ecosystem Bridge SDK

The Ecosystem Bridges track now has a reusable construction SDK:

```text
diagprint-bridge
    reusable adapter mechanics

diagprint-error-stack
    first concrete adapter
```

`diagprint-bridge` builds on the core `InteropDiagnostic` protocol and M4
relationship graph. `diagprint-error-stack` proves that SDK against a real
structured error ecosystem, including grouped reports and privacy-aware
attachment handling.

Attachment content is private by default. Printable attachment text requires
explicit opt-in; opaque attachment values are not exported.

Applications that only need core diagprint do not depend on `error-stack`.

For v0.8 applications building ecosystem adapters:

```toml
[dependencies]
diagprint = "0.8"
diagprint-bridge = "0.8"

# Only when error-stack integration is required:
diagprint-error-stack = "0.8"
```


## Remediation evidence replay

diagprint can bind a successful guarded remediation receipt to an exact
adjacent transition in tamper-evident diagnostic history, then replay that
evidence later without reapplying edits.

```text
diagprint replay <HISTORY> <FINGERPRINT>
diagprint replay <HISTORY> <FINGERPRINT> --format json
diagprint history remediation-verify <HISTORY>
```

Replay distinguishes observed diagnostic lifecycle evidence from causal claims.
A diagnostic may be reported as reappearing after an observed resolution
associated with a verified remediation, while remediation causation,
recurrence root cause, and Git causation remain explicitly **not established**.

Replay is read-only: it never invokes `FixPlan::apply` or executes shell
commands.

## Installation

```toml
[dependencies]
diagprint = "0.8"
```

`diagprint` v0.8 uses Rust 2024 and supports Rust **1.85 and newer**.

### CLI

```bash
curl -fsSL https://raw.githubusercontent.com/darkstardevx/diagprint/main/install.sh | sh
```

Downloads the latest release for your platform (Linux or macOS,
x86_64 or aarch64), verifies its SHA-256 checksum, and installs
`diagprint` to `~/.local/bin`. Or, since it's also on crates.io:

```bash
cargo install diagprint
```

## Optional Features

All optional features are disabled by default.

```toml
[dependencies]
diagprint = {
    version = "0.8",
    features = [
        "artifact-store",
        "derive",
        "compression",
        "html",
        "cybercore",
        "terminal-docs",
        "anyhow",
        "tracing",
        "miette",
        "codespan-reporting",
        "ariadne",
        "annotate-snippets",
    ]
}
```

| Feature | Purpose |
| --- | --- |
| `artifact-store` | append-only artifact generations with filesystem locking |
| `derive` | typed diagnostic derive support |
| `compression` | gzip and Zstandard report compression |
| `html` | HTML diagnostic and report rendering |
| `cybercore` | Cybercore theme integration |
| `terminal-docs` | terminal documentation retrieval and highlighting |
| `anyhow` | anyhow diagnostic integration |
| `tracing` | tracing subscriber integration |
| `miette` | miette interoperability |
| `codespan-reporting` | codespan-reporting interoperability |
| `ariadne` | Ariadne structured bridge |
| `annotate-snippets` | annotate-snippets structured bridge |

## Quick Start

```rust
use diagprint::{Reporter, Severity};

fn main() -> std::io::Result<()> {
    let reporter = Reporter::builder()
        .application("myapp")
        .min_severity(Severity::Info)
        .build()?;

    let diagnostic = reporter
        .error("Network initialization failed")
        .code("NET-001")
        .cause("failed to open network interface")
        .note("fallback networking is unavailable")
        .help("check interface permissions and driver state");

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
    )
    .secondary_label(
        "defaults.toml",
        4,
        Some(1),
        Some(7),
        Some("default declared here"),
    );
```

Primary and secondary labels remain structurally distinct through rendering and
interop.

The terminal renderer displays them differently so related context does not
look like an additional primary failure.

## Virtual and In-Memory Sources

Source text does not need to exist on disk.

```rust
let reporter = Reporter::builder()
    .application("editor")
    .source(
        "memory://editor/main.rs",
        "let answer = old_value();\n",
    )
    .build()?;

let diagnostic = reporter
    .error("Invalid value")
    .label(
        "memory://editor/main.rs",
        1,
        Some(14),
        Some(9),
        Some("value"),
    );

reporter.emit(&diagnostic)?;
```

`SourceCache` stores virtual or generated source text:

```rust
use diagprint::SourceCache;

let cache = SourceCache::new();

cache.insert(
    "memory://generated.rs",
    "fn generated() {}\n",
);
```

Source names are matched exactly.

When source-aware terminal rendering is used, cached source takes precedence
over filesystem fallback.

Cloned `SourceCache` handles share the same underlying source state.

## Immutable Source Snapshots

Mutable editor buffers can change after a diagnostic is created.

`SourceSnapshot` freezes the source view at a point in time while source text
itself remains shared through `Arc`.

```rust
let cache = reporter.source_cache();

let snapshot = cache.snapshot();

let diagnostic = reporter
    .error("old diagnostic")
    .label(
        "memory://editor/main.rs",
        1,
        Some(14),
        Some(9),
        Some("original value"),
    )
    .bind_source_revisions(&snapshot);
```

Later mutations to the live cache do not change the snapshot.

## Source Revisions

Every cached source tracks a `SourceRevision`.

```rust
let revision = cache.insert_revisioned(
    "memory://editor/main.rs",
    "let answer = old_value();\n",
);

println!("revision: {revision}");
```

Revisions are tracked independently per source.

Every insertion advances the revision, including replacement with identical
text.

Revision history survives removal and clearing so an old revision cannot be
silently reused after a source is recreated.

## Revision-Bound Diagnostics

A diagnostic can bind its source locations to the source revision it was
created from.

```rust
let snapshot = reporter.source_snapshot();

let diagnostic = reporter
    .error("Invalid editor value")
    .label(
        "memory://editor/main.rs",
        1,
        Some(14),
        Some(9),
        Some("old value"),
    )
    .bind_source_revisions(&snapshot);
```

If the live source later changes, a revision-aware terminal render fails
closed.

Instead of underlining unrelated newer text, it reports the mismatch:

```text
! stale source: r1 != r2
```

No misleading newer source excerpt is shown.

Revision-unbound diagnostics retain the existing source behavior.

## Captured Diagnostics

`CapturedDiagnostic` pairs a diagnostic with the immutable source snapshot that
belongs to it.

```rust
let captured = reporter.capture(
    reporter
        .error("editor diagnostic")
        .label(
            "memory://editor/main.rs",
            1,
            Some(14),
            Some(9),
            Some("source at diagnostic time"),
        ),
);
```

The live buffer may then continue changing:

```rust
reporter.register_source(
    "memory://editor/main.rs",
    "let answer = new_value();\n",
);

assert!(
    captured.is_stale(
        &reporter.source_cache()
    )
);
```

The captured diagnostic still renders against its original immutable source:

```rust
reporter.emit_captured(&captured)?;
```

Source text stays outside `Diagnostic` serialization.

## Source Providers

Integrations which own in-memory source text can expose it through
`SourceProvider`.

A reporter can register those sources:

```rust
reporter.register_sources(&provider);
```

Or a builder can import them:

```rust
let reporter = Reporter::builder()
    .sources_from(&provider)
    .build()?;
```

The Ariadne and annotate-snippets bridges implement this handoff.

## GitHub Actions Annotations

`diagprint` can emit native GitHub Actions workflow-command annotations.

```rust
let diagnostic = reporter
    .error("Cannot combine incompatible values")
    .code("E-TYPE")
    .label(
        "src/main.rs",
        12,
        Some(9),
        Some(5),
        Some("numeric value"),
    )
    .secondary_label(
        "src/lib.rs",
        4,
        Some(5),
        Some(8),
        Some("string declaration"),
    );

reporter.emit_github_actions(&diagnostic)?;
```

Severity mapping:

| diagprint | GitHub Actions |
| --- | --- |
| Trace | notice |
| Debug | notice |
| Info | notice |
| Warning | warning |
| Error | error |
| Fatal | error |

Primary labels retain the diagnostic severity.

Secondary labels are emitted as notices so related source locations remain
visible without appearing as additional failures.

Workflow command data and properties are escaped before output.

## SARIF 2.1.0

`SarifRenderer` produces SARIF for GitHub Code Scanning and other SARIF 2.1.0
consumers.

```rust
use diagprint::render::SarifRenderer;

let first = reporter
    .error("Type mismatch")
    .code("E-TYPE")
    .label(
        "src/main.rs",
        12,
        Some(9),
        Some(5),
        Some("numeric value"),
    )
    .secondary_label(
        "src/lib.rs",
        4,
        Some(5),
        Some(8),
        Some("declared here"),
    );

let second = reporter
    .warning("Deprecated configuration")
    .code("W-CONFIG")
    .label(
        "src/config.rs",
        7,
        Some(1),
        Some(12),
        Some("deprecated setting"),
    );

SarifRenderer.write_many(
    "target/diagprint.sarif",
    [&first, &second],
)?;
```

SARIF output includes:

- SARIF 2.1.0 metadata;
- deterministic rule IDs;
- deterministic rule indices;
- severity mapping;
- primary source locations;
- related source locations;
- exclusive SARIF end-column ranges;
- notes;
- help;
- cause information;
- diagprint source revision metadata.

No synthetic fingerprints are invented.

`write_many()` writes one complete SARIF document rather than appending
independent JSON documents.

## Built-In Output Formats

The same diagnostic data can be rendered as:

- terminal output;
- plain text;
- JSON;
- Markdown;
- GitHub Actions annotations;
- SARIF 2.1.0.

```rust
use diagprint::render::{
    GithubActionsRenderer,
    JsonRenderer,
    MarkdownRenderer,
    PlainRenderer,
    Renderer,
    SarifRenderer,
    TerminalRenderer,
};
```

Diagnostic construction stays independent from presentation.

## Generic Diagnostic Interop

`diagprint` provides a dependency-free interoperability protocol for structured
diagnostics.

The generic protocol can preserve:

- severity;
- diagnostic codes;
- messages;
- help;
- notes;
- source labels;
- primary and secondary label roles;
- causes;
- documentation links;
- related diagnostics.

Generic interoperability deliberately does **not** grant remediation trust.

An integration that wants automatic edits must establish remediation trust
separately.

## anyhow

Enable:

```toml
features = ["anyhow"]
```

The anyhow adapter can convert error context into structured diagprint
diagnostics while preserving the error chain.

## tracing

Enable:

```toml
features = ["tracing"]
```

The tracing integration connects structured tracing events with diagprint
reporting.

## miette

Enable:

```toml
features = ["miette"]
```

The miette integration maps compatible diagnostic metadata into diagprint's
structured interoperability model.

## codespan-reporting

Enable:

```toml
features = ["codespan-reporting"]
```

Codespan diagnostics are routed through the generic interoperability layer.

## Ariadne

Enable:

```toml
features = ["ariadne"]
```

`AriadneBridge` captures structured source and label metadata once and can
produce both:

- a real Ariadne report;
- a diagprint interoperability diagnostic.

The bridge does not parse rendered Ariadne terminal text and does not rely on
private Ariadne internals.

Ariadne source spans are resolved into one-based source locations for
diagprint.

## annotate-snippets

Enable:

```toml
features = ["annotate-snippets"]
```

The annotate-snippets bridge:

- preserves primary and context labels;
- validates byte ranges;
- rejects invalid UTF-8 boundaries;
- converts byte spans into one-based line and column locations;
- exposes its in-memory sources through `SourceProvider`.

## Rustc Diagnostics

`diagprint` can ingest structured rustc JSON diagnostics.

The compiler integration can preserve information including:

- severity;
- error codes;
- source spans;
- child diagnostics;
- structured suggestions;
- applicability;
- compiler source edits;
- rendered compiler context when available.

Compiler edits require explicit trusted source-root hydration before they can
participate in remediation.

## Cargo Intelligence

Cargo ingestion can track information including:

- workspace packages;
- exact package versions;
- workspace membership;
- targets;
- resolved dependencies;
- renamed dependencies;
- compiler artifacts;
- build-script results;
- build completion;
- build summaries.

Unknown Cargo messages are preserved for forward compatibility.

## Diagnostic Suggestions

Diagnostics can carry structured suggestions.

```rust
use diagprint::{
    Applicability,
    Edit,
    Suggestion,
    TextRange,
};

let suggestion =
    Suggestion::new(
        "Replace deprecated value",
    )
    .applicability(
        Applicability::MachineApplicable,
    )
    .edit(
        Edit::replace(
            "config.toml",
            TextRange::new(10, 13),
            "old",
            "new",
        ),
    );
```

Applicability levels are:

```rust
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Manual,
}
```

Only guarded, machine-applicable structured edits are eligible for automatic
application.

## Fixer

Validate without writing:

```rust
use diagprint::Fixer;

let check =
    Fixer::new()
        .check(&diagnostic)?;
```

Apply validated fixes:

```rust
let report =
    Fixer::new()
        .backups(true)
        .apply(&diagnostic)?;
```

Interactive mode:

```rust
Fixer::new()
    .backups(true)
    .apply_interactive(
        &diagnostic,
    )?;
```

Before mutation, diagprint validates:

- applicability;
- expected source contents;
- edit ranges;
- UTF-8 boundaries;
- overlapping edits;
- duplicate insertion positions;
- filesystem state.

Stale edits are rejected instead of guessed.

## FixPlan

`FixPlan` supports transaction-wide multi-file remediation.

The remediation flow:

1. validates all affected files;
2. prepares all resulting contents;
3. creates recovery state;
4. performs transaction writes;
5. rolls back observed failures;
6. optionally performs post-fix verification;
7. rolls back when verification fails.

Portable crash-atomic multi-file writes are **not** claimed.

## Post-Fix Verification

Fix plans can declare structured verification requirements after application.

Verification remains declarative.

`diagprint` does not automatically execute arbitrary shell commands as part of
fix application or verification.

## Suggested Commands

Suggestions may contain advisory commands:

```rust
use diagprint::SuggestedCommand;

let command =
    SuggestedCommand::new(
        "cargo check",
    )
    .explanation(
        "Verify the project after applying the edit",
    );
```

Suggested commands are **never executed automatically**.

## Documentation Intelligence

`DocumentationResolver` supports version-aware documentation resolution using
Cargo metadata and lockfiles.

Supported documentation targets include:

- Rust error documentation;
- Cargo Book pages;
- docs.rs package documentation;
- custom documentation links.

Ambiguous package versions fail closed instead of guessing a docs.rs version.

## Terminal Documentation

Enable:

```toml
features = ["terminal-docs"]
```

Then:

```rust
use diagprint::{
    DocumentationLink,
    TerminalDocViewer,
};

let link =
    DocumentationLink::rust_error(
        "E0277",
    );

TerminalDocViewer::new()
    .width(96)
    .open_and_print(&link)?;
```

The terminal documentation viewer:

- accepts HTTP and HTTPS documentation URLs;
- limits remote document size;
- sanitizes terminal control characters;
- converts HTML to terminal-readable text;
- extracts code examples;
- syntax-highlights code.

The current documentation viewer uses blocking I/O.

## Themes

Terminal presentation is customizable.

```rust
use diagprint::{
    Style,
    Theme,
};

let theme = Theme {
    border: Style::rgb(
        20,
        185,
        181,
    ),
    patch_add: Style::rgb(
        100,
        255,
        100,
    ),
    patch_remove: Style::rgb(
        255,
        80,
        100,
    ),
    ..Theme::default()
};
```

Styles support:

- standard ANSI colors;
- ANSI-256 colors;
- RGB foregrounds and backgrounds;
- hex colors;
- bold;
- dim;
- italic;
- underline.

`color(false)` remains authoritative and disables ANSI styling regardless of
theme configuration.

## Cybercore Integration

Enable:

```toml
features = ["cybercore"]
```

Use the active Cybercore theme:

```rust
use diagprint::Theme;

let theme =
    Theme::cybercore();
```

Named Cybercore themes are also supported.

The integration consumes Cybercore's semantic palette rather than duplicating
theme values inside diagprint.

## Rotation

File reports support:

- size-based rotation;
- hourly rotation;
- daily rotation;
- retention cleanup.

```rust
use diagprint::{
    RotationCadence,
    RotationPolicy,
};

let policy = RotationPolicy {
    cadence:
        RotationCadence::Daily,
    ..Default::default()
};
```

## Compression

Enable:

```toml
features = ["compression"]
```

Supported compression formats:

```rust
use diagprint::Compression;

// Compression::Gzip
// Compression::Zstd
```

## Safety Model

`diagprint` deliberately separates diagnostic presentation from mutation.

Rendering never modifies source files.

Automatic remediation is restricted to structured edits that:

1. are marked `MachineApplicable`;
2. still match expected source contents;
3. use valid UTF-8 boundaries;
4. do not overlap;
5. satisfy declared preconditions.

Additional safeguards include:

- stale-edit rejection;
- guarded insertion requirements;
- transaction-wide validation;
- recovery state before writes;
- rollback on observed write failures;
- optional post-fix verification;
- verification rollback;
- trusted-root requirements for hydrated compiler edits;
- fail-closed documentation version resolution.

Suggested shell commands are never automatically executed.

Revision-aware source rendering also fails closed when source identity no longer
matches the diagnostic.

## Minimum Supported Rust Version

The minimum supported Rust version is:

```text
Rust 1.85
```

CI performs an MSRV-aware fresh dependency resolution and checks all targets and
all features on Rust 1.85.

## Development

Install repository hooks once per clone:

    ./scripts/install-hooks

Rust implementation is plan-first:

    ./scripts/plan new M4-causal-graph
    # edit the generated plan
    ./scripts/plan approve
    git add .plans/
    git commit

Only after the Approved plan is committed should Rust implementation begin.

Development gates:

    ./scripts/gate.sh precommit
    ./scripts/gate.sh fast
    ./scripts/gate.sh forensics
    ./scripts/gate.sh full

The repository-owned pre-commit hook checks staged whitespace and, for code
changes, all-target/all-feature Cargo check plus strict Clippy. Rust changes
also require an Approved plan already committed in HEAD.

Bacon is optional. The committed `bacon.toml` provides check, clippy, test,
forensics, and precommit jobs.

Git's `--no-verify` remains a human emergency escape hatch; coding agents are
instructed not to use it.

## Examples

Core examples:

```bash
cargo run --example basic
cargo run --example error_chain
cargo run --example intelligence
```

Revision-aware source examples:

```bash
cargo run --example virtual_source
cargo run --example source_snapshot
cargo run --example source_revision
cargo run --example revision_bound_diagnostic
cargo run --example captured_diagnostic
```

CI output examples:

```bash
cargo run --example github_actions
cargo run --example sarif
```

Interop examples:

```bash
cargo run \
    --features ariadne \
    --example ariadne_integration

cargo run \
    --features annotate-snippets \
    --example annotate_snippets_integration
```

Other optional examples:

```bash
cargo run \
    --features cybercore \
    --example cybercore

cargo run \
    --features terminal-docs \
    --example terminal_docs
```

## v0.6

### Revision-Aware Diagnostics and CI Output

v0.6 adds:

- distinct primary and secondary label rendering;
- virtual source caching;
- generic source-provider handoff;
- immutable source snapshots;
- per-source revision tracking;
- revision-bound source locations;
- stale-source detection;
- fail-closed stale-source terminal rendering;
- captured diagnostics;
- Ariadne structured interoperability;
- annotate-snippets structured interoperability;
- GitHub Actions annotations;
- SARIF 2.1.0 rendering;
- hardened Rust 1.85 validation across all targets and features.

## v0.5

### Interoperability and Transactional Remediation

v0.5 added:

- Rust 2024;
- Rust 1.85 MSRV;
- anyhow integration;
- typed-error metadata;
- tracing integration;
- rustc/Cargo structured ingestion;
- version-aware documentation;
- FixPlan;
- transaction-wide multi-file remediation;
- post-fix verification;
- Cargo intelligence;
- miette interoperability;
- codespan-reporting interoperability;
- generic dependency-free diagnostic interoperability.

## Roadmap

Potential future work includes:

- asynchronous and nonblocking report output;
- asynchronous terminal documentation retrieval;
- richer structured diff presentation;
- additional structured fix sources.

## Repository

https://github.com/darkstardevx/diagprint

## License

Licensed under either:

- Apache License, Version 2.0
- MIT License

at your option.
