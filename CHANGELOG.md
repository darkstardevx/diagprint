# Changelog

All notable changes to `diagprint` will be documented in this file.

The project follows Semantic Versioning.

## [Unreleased]

### Changed

- Repository infrastructure and continuous integration.
- Preparing terminal renderer improvements for v0.3.1.

## [0.3.0] - 2026-09-14

### Added

- Width-aware terminal diagnostic renderer.
- Source snippets with line numbers and caret highlighting.
- Hierarchical diagnostic causes.
- `std::error::Error::source()` chain capture.
- Size-based log rotation.
- Hourly and daily log rotation.
- Rotated-file retention.
- Optional gzip compression.
- Optional Zstandard compression.
- Thread-safe file writes.
- JSON renderer.
- Markdown renderer.
- Plain-text renderer.
- Application, PID, hostname, timestamp, session ID, and report ID metadata.
- Severity filtering.

## [0.2.0]

### Added

- Renderer architecture.
- JSON and Markdown output.
- Initial source highlighting.
- Error causes.
- Rotation abstraction.

## [0.1.0]

### Added

- Initial structured diagnostic model.
- Terminal rendering.
- Severity levels.
- File output.
- Session and report IDs.