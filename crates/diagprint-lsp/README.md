# diagprint-lsp

Language Server Protocol integration for the `diagprint` diagnostics lifecycle framework.

`diagprint-lsp` converts structured, revision-aware diagnostics into LSP
diagnostics and guarded remediation into versioned code actions.

## Highlights

- UTF-8, UTF-16, and UTF-32 position encoding
- document/version tracking
- grouped `publishDiagnostics` payloads
- report-wide code action generation
- immutable source snapshot support
- stale-source and stale-document rejection
- guarded structured remediation
- no parsing of rendered terminal output

Source revisions from `diagprint` and external LSP document versions remain
separate so stale edits fail closed instead of being guessed.

## Installation

    [dependencies]
    diagprint = "0.7"
    diagprint-lsp = "0.7"

## Main API

The primary entry point is `LspAdapter`.

Supporting types include:

- `DocumentMap`
- `LspDocument`
- `PositionEncoding`
- `LspError`

`diagprint-lsp` also re-exports `lsp-types`.

## Safety

Code actions are generated only when source and document state can be validated.
Manual, placeholder, stale, overlapping, or otherwise unsafe remediation is
rejected rather than silently converted into an edit.

## MSRV

Rust 1.85 or newer.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
