# Changelog

All notable changes to `diagprint` are documented in this file.

The project follows Semantic Versioning.

## [Unreleased]

### Added

- Added policy-aware GitHub Actions and SARIF rendering with repository-relative or omitted source paths and optional text redaction, while preserving historical renderer behavior by default.
- Added a shared `ExportPolicy` with explicit controls for free-form text, source paths, structured attributes, remediation metadata, documentation URLs, and process metadata; existing JSON/JSONL behavior retains conservative safe defaults.
- Added repository-relative source-path export with fail-closed filename fallback for locations outside the configured root.
- Added opt-in structured attribute export modes including common sensitive-key redaction while preserving non-sensitive typed values.
- Added an export-safe diagnostic representation used by JSON and JSON Lines output; default external serialization omits arbitrary attribute values, remediation source text, and suggested command contents while retaining structural counts.
- Added dependency-free path, URL, and common sensitive-key sanitization primitives for external diagnostic boundaries.
- Added explicit OpenTelemetry attribute privacy controls with `AttributeExport::{Omit, Redact, Full}`; arbitrary structured diagnostic attributes are omitted by default.
- Full OpenTelemetry attribute export preserves supported typed values and losslessly falls back to decimal strings for integers outside OpenTelemetry's signed 64-bit range.
- Added typed `DiagnosticAttribute` / `DiagnosticValue` metadata so structured booleans, integers, floating-point values, and strings no longer need to be flattened into diagnostic notes.
- The tracing adapter now preserves ordinary event fields as typed diagnostic attributes while retaining reserved diag.code, diag.help, diag.note, message, and error-chain handling.
- Added opt-in `TracingLayer::with_span_path_attribute` support for recording the active tracing span hierarchy as the structured `tracing.span_path` diagnostic attribute using the event scope provided by `tracing-subscriber`.
- Added the `diagprint-otel` companion crate for privacy-aware OpenTelemetry diagnostic events and tracing span integration.
- Telemetry export is metadata-first by default: diagnostic messages are redacted while help, notes, causes, source paths, hostname, and PID are omitted unless explicitly enabled.
- Added native OpenTelemetry Span and tracing-opentelemetry recording for single diagnostics and DiagnosticReport batches without coupling diagprint to a collector or exporter SDK.
- Added the runtime-independent `DiagnosticSink` contract with buffered writer and newline-delimited JSON production sinks.
- Added the `diagprint-async` companion crate with strictly bounded diagnostic queues, ordered flush/shutdown, worker failure propagation, and explicit Block, Reject, and severity-aware DropNewest backpressure.
- Async DropNewest queues protect diagnostics above their configured drop threshold instead of silently discarding them under overload.
- Added first-class batch/report output with deterministic report status and configurable severity-based exit decisions.
- Added valid multi-diagnostic JSON, Markdown, GitHub Actions, and SARIF report rendering plus Reporter batch emission APIs.
- Added grouped LSP report publishing and report-wide CodeAction generation against immutable source snapshots.
- Added the `diagprint-lsp` companion crate with revision-aware LSP diagnostic conversion, UTF-8/UTF-16/UTF-32 position encoding, external document-version tracking, and safe versioned CodeActions.
- LSP remediation fails closed for stale guards, missing document versions, invalid ranges, overlapping edits, placeholder/manual fixes, and suggested commands.
- Introduced a Cargo workspace with the optional `diagprint-derive` procedural
  macro crate.
- Expanded `diagprint-derive` with enum and variant metadata, inheritance/overrides,
  structured suggestions, and fail-closed rejection of machine-applicable suggestions
  that do not carry guarded edits.
- Added derive-based typed diagnostic metadata with structured primary and
  secondary source labels.
- Added `DiagnosticReport` and `SeverityCounts` for aggregation, filtering,
  deterministic ordering, and severity summaries.
- Added `ResultDiagnosticExt` for converting typed `Result` errors directly
  into diagnostics or revision-aware captured diagnostics.
- Added safe-by-default `Sensitive<T>` values and explicit redaction policy
  primitives for future exporters and integrations.



### Fixed

- Corrected the tracing integration so structured active span hierarchy is exposed as `tracing.span_path` rather than being misrepresented as `tracing-error::SpanTrace`.
- Removed the unused `tracing-error` feature, dependency, and `TracingErrorLayer` re-export.
- Bounded retained tracing emission failures so a permanently failing output destination cannot cause unbounded diagnostic-layer memory growth.
- Reserved the generated `tracing.span_path` attribute when structured span-path capture is enabled, preventing event-field collisions from overriding library-generated context.
- Reporter stdout emission now propagates `io::Error` instead of relying on panic-prone standard printing macros.
- JSON and SARIF renderers now expose fallible serialization entry points; the existing string-rendering APIs degrade to valid format-specific error documents instead of panicking on serialization failure.
- Removed avoidable internal panic sites from cause-chain construction, documentation version resolution, ANSI truncation, and SARIF rule lookup.
- Source revision exhaustion remains deliberately fail-stop rather than wrapping or saturating, preserving the guarantee that stale source revisions are never silently reused.
- URL sanitization now strips credentials from scheme-relative hierarchical URLs in addition to ordinary hierarchical URLs.
- CI now enforces strict rustdoc warnings, tracing-only feature coverage, full MSRV tests, and root feature-isolation checks.
- Pinned `yoke-derive` to 0.8.2 on the `terminal-docs` feature path so fresh dependency resolution remains compatible with the Rust 1.85 MSRV; 0.8.3 is currently selectable by Cargo's MSRV-aware resolver but does not compile on Rust 1.85.


### Planned

- Nonblocking and asynchronous report output.
- Async terminal documentation retrieval.
- Additional renderers.
- Richer structured diff presentation.

## [0.6.0] - 2026-09-15

### Changed

- Enforced the declared Rust 1.85 MSRV across all targets and optional
  features using MSRV-aware dependency resolution.
- Kept terminal documentation compatible with Rust 1.85 by using
  `scraper` 0.25.x.
- Reworked internal let-chain expressions into Rust 1.85-compatible
  control flow without changing behavior.

### Added

- SARIF 2.1.0 rendering for GitHub Code Scanning and other SARIF consumers,
  including deterministic rules, primary and related locations, exclusive
  source ranges, severity mapping, source revisions, notes, help, and causes.

- Dependency-free `GithubActionsRenderer` for native GitHub Actions
  `notice`, `warning`, and `error` annotations with source ranges, command
  escaping, diagnostic codes, help, notes, and secondary related locations.

- `CapturedDiagnostic` bundles a diagnostic with its immutable source
  snapshot for stable editor/LSP storage, stale detection, and exact rendering.

- Revision-bound diagnostic source locations with stale-source detection.
- Terminal rendering fails closed when a diagnostic revision does not match
  the available live source, preventing misleading highlights on newer text.

- Per-source `SourceRevision` tracking with immutable revision-aware
  snapshots and stale/current detection for mutable editor buffers.

- Immutable `SourceSnapshot` views for rendering diagnostics against the
  exact in-memory source text that existed when the snapshot was captured.

- Generic `SourceProvider` handoff for integrations which own in-memory
  sources, with Ariadne and annotate-snippets bridges implementing it.

- Shared `SourceCache` for virtual and in-memory source text.
- Terminal rendering can prefer cached source and fall back to filesystem
  source without changing the existing `Renderer` API.
- Reporter-level source registration for generated files, editor buffers, and
  other sources which may never exist on disk.

- Structured Ariadne bridge producing both Ariadne reports and diagprint
  interoperability diagnostics without rendered-output parsing.
- Structured annotate-snippets bridge with validated byte spans, Unicode-safe
  location conversion, and primary/context label preservation.
- Distinct terminal rendering for primary and secondary source labels.
- Plain-text source-label rendering with explicit label roles.
- Markdown source-label rendering with explicit label roles.
- Secondary source locations remain explicit in JSON while primary labels
  retain the backward-compatible default representation.



## [0.5.0] - 2026-09-15

### Added

#### Rust 2024

- Migrated diagprint to Rust 2024.
- Declared MSRV Rust 1.85.

#### Ecosystem Interoperability

- Anyhow diagnostic adapter.
- Typed-error and thiserror-compatible metadata.
- tracing-subscriber integration.
- miette diagnostic interoperability.
- codespan-reporting interoperability.
- Dependency-free generic diagnostic interop protocol.
- Structured primary and secondary source-label kinds.

#### Compiler and Cargo Intelligence

- Structured rustc JSON diagnostic importer.
- Cargo compiler-message ingestion.
- Trusted source-root edit hydration.
- Structured rustc suggestions and applicability.
- Cargo metadata workspace ingestion.
- Exact package and version tracking.
- Workspace membership and target metadata.
- Resolved dependency graph and renamed dependencies.
- Compiler artifact ingestion.
- Build-script result ingestion.
- Cargo build completion and build summaries.
- Forward-compatible unknown Cargo message preservation.

#### Documentation Intelligence

- Version-aware DocumentationResolver.
- Cargo.lock package-version discovery.
- Cargo metadata package-version discovery.
- Exact docs.rs links.
- Rust toolchain-version documentation pinning.
- Fail-closed ambiguous-version handling.

#### Transactional Remediation

- FixPlan.
- Declarative preconditions.
- Transaction-wide multi-file edit preparation.
- Rollback-on-error writes.
- Post-apply verification.
- Verification rollback.
- Optional backups.
- Rollback failure reporting.
- Shared remediation engine for Fixer and FixPlan.
- Unguarded automatic insert rejection.

### Changed

- Fixer multi-file application now uses transaction-wide remediation.
- Ecosystem integrations consume structured protocols instead of rendered text.
- Diagnostic interoperability is separated from remediation guarantees.
- Cargo.lock is tracked for deterministic dependency resolution.
- Source label priority is preserved structurally.

### Security

- Automatic remediation remains restricted to guarded machine-applicable edits.
- Suggested commands are never automatically executed.
- Imported compiler edits require explicit trusted-root hydration.
- Ambiguous documentation versions fail closed.
- FixPlan verification remains declarative and does not execute shell commands.

## [0.4.0] - 2026-09-14

### Added

#### Diagnostic Intelligence

- Structured `Suggestion` API.
- `Applicability` classifications:
  - `MachineApplicable`
  - `MaybeIncorrect`
  - `HasPlaceholders`
  - `Manual`
- Structured `DocumentationLink` API.
- Structured `SuggestedCommand` API.
- Structured `Edit` operations.
- `TextRange` support.
- Replace, insert, and delete edit operations.
- Multiple edits per suggestion.
- Multiple suggestions per diagnostic.

#### Fix Engine

- `Fixer`.
- `Fixer::check()` dry-run validation.
- `Fixer::preview()`.
- `Fixer::apply()`.
- `Fixer::apply_interactive()`.
- `FixCheck`.
- `FixPreview`.
- `FixReport`.
- Optional backup creation.
- Configurable backup suffixes.
- Atomic replacement writes.
- UUIDv7 temporary filenames.
- Multi-file fix preparation.
- Multiple non-overlapping edits per file.

#### Fix Safety

- Machine-applicable gating.
- Exact expected-content verification.
- Stale-edit rejection.
- Stale-insert rejection.
- UTF-8 boundary validation.
- Invalid-range rejection.
- Overlapping-edit rejection.
- Duplicate insert-position rejection.
- Pre-application filesystem validation.
- Interactive apply action hidden when validation fails.
- Suggested commands remain informational and are never auto-executed.

#### Terminal Rendering

- Structured `SUGGESTION` sections.
- Patch display.
- Added-line styling.
- Removed-line styling.
- Documentation-link styling.
- Suggested-command styling.
- Applicability display.
- Automatic/manual fix status.
- New theme roles for diagnostic intelligence.

#### Terminal Documentation

- Optional `terminal-docs` feature.
- In-terminal documentation retrieval.
- HTTP and HTTPS URL validation.
- HTML-to-terminal text rendering.
- Markdown documentation rendering.
- Code-example extraction.
- 24-bit ANSI syntax highlighting.
- Configurable terminal width.
- Configurable syntax theme.
- Configurable maximum code blocks.
- Configurable HTTP timeout.
- Configurable document-size limit.
- 2 MiB default remote document limit.
- Terminal control-character sanitization.
- Offline documentation-rendering regression tests.
- Interactive `[D]ocs` action.

#### Documentation Helpers

- `DocumentationLink::docs_rs()`.
- `DocumentationLink::rust_error()`.
- `DocumentationLink::cargo_book()`.

#### Examples

- Diagnostic intelligence demonstration.
- Safe application demonstration.
- Interactive fixer demonstration.
- Terminal documentation demonstration.

### Changed

- `Diagnostic` now carries structured suggestions.
- Terminal themes now include diagnostic-intelligence style roles.
- Interactive fixing revalidates current file contents before exposing the
  apply action.
- Interactive mode now separates diagnostic presentation from fix execution.
- Atomic temporary writes now use unique UUIDv7 filenames.
- Documentation rendering sanitizes untrusted terminal control characters.
- Terminal documentation is isolated behind an optional feature.
- Crates.io publishing is intentionally disabled while the optional Cybercore
  integration depends on a Git source.

### Fixed

- Prevented stale file contents from being overwritten by an outdated fix.
- Prevented overlapping structured edits from being applied.
- Prevented byte offsets inside UTF-8 code points from being edited.
- Prevented invalid edit ranges from reaching the write phase.
- Prevented unavailable interactive actions from being offered.
- Prevented terminal escape sequences from remote documentation from reaching
  the terminal unchanged.
- Prevented unexpectedly large documentation responses from being rendered.
- Removed duplicated full suggestion presentation from interactive fix mode.

### Security

- Remote documentation accepts only HTTP and HTTPS URLs.
- Remote terminal control characters are sanitized before display.
- Remote documents are size-limited.
- Suggested commands are never automatically executed.
- Automatic edits fail closed when expected content no longer matches.

## [0.3.1] - 2026-09-14

### Added

- Unicode-aware terminal display width.
- Automatic terminal-width detection.
- Intelligent text wrapping for diagnostic messages.
- Wrapped causes, notes, help text, and metadata.
- Intelligent source-window clipping around highlighted ranges.
- Source target-line gutter markers.
- Wrapped multiline source labels.
- ANSI-safe terminal width calculations.
- Renderer regression tests.
- Golden-output terminal renderer tests.
- Public `Theme` API.
- Public `Style` API.
- Public `SeverityTheme` API.
- True-color RGB foreground styles.
- True-color RGB background styles.
- ANSI-256 foreground and background styles.
- Hex RGB parsing.
- Bold, dim, italic, and underline modifiers.
- Optional Cybercore integration.
- Cybercore active-theme support.
- Cybercore named-theme lookup.
- Cybercore theme discovery.
- Cybercore theme existence checks.
- Cybercore safe active-theme fallback.
- Cybercore severity showcase example.

### Changed

- Terminal source rendering follows the highlighted source region instead of
  blindly truncating from the beginning of the line.
- Terminal renderer gutters and annotations were redesigned for clearer
  diagnostics.
- `CAUSE`, `NOTE`, and `HELP` use consistent layout and wrapping.
- Terminal colors are routed through the theme system rather than hardcoded
  renderer logic.
- `color(false)` acts as a global styling override regardless of the selected
  theme.
- Cybercore colors are consumed semantically from the Cybercore schema rather
  than duplicated inside `diagprint`.

### Fixed

- Incorrect display widths for Unicode terminal content.
- Long source lines hiding the actual diagnostic location.
- Long unbroken strings disappearing or truncating incorrectly.
- Source labels overflowing narrow diagnostic boxes.
- ANSI sequences affecting box-width calculations.

## [0.3.0] - 2026-09-14

### Added

- Structured diagnostics.
- Severity levels.
- Source locations.
- Source labels.
- Hierarchical causes.
- `std::error::Error::source()` chain capture.
- Width-aware boxed terminal renderer.
- Plain-text renderer.
- JSON renderer.
- Markdown renderer.
- UUIDv7 session identifiers.
- UUIDv7 report identifiers.
- Application metadata.
- PID metadata.
- Hostname metadata.
- Timestamp metadata.
- File reporting.
- Size-based rotation.
- Hourly rotation.
- Daily rotation.
- Retention cleanup.
- Optional gzip compression.
- Optional Zstandard compression.
- Shared synchronization for file writes.
