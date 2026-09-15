# Changelog

All notable changes to `diagprint` are documented in this file.

The project follows Semantic Versioning.

## [Unreleased]

### Planned

- Dedicated `anyhow` integration
- Expanded `thiserror` integration
- `tracing` integration
- Nonblocking and asynchronous report output
- Async terminal documentation retrieval
- Additional renderers
- Richer structured diff presentation

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
