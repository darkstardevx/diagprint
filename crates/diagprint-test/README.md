# diagprint-test

Testing helpers for the `diagprint` diagnostics lifecycle framework.

`diagprint-test` provides assertions and deterministic snapshot helpers over
the structured diagnostic model rather than parsing terminal output.

## Highlights

- diagnostic assertions
- report assertions
- severity and code checks
- source-label assertions
- suggestion assertions
- structured attribute assertions
- canonical fingerprint and digest assertions
- semantic delta assertions
- deterministic diagnostic/report snapshots
- file snapshot helpers

## Installation

    [dev-dependencies]
    diagprint = "0.7"
    diagprint-test = "0.7"

## Common imports

    use diagprint_test::prelude::*;

The prelude exposes the primary assertion traits and snapshot helpers.

## Testing philosophy

Tests inspect `diagprint`'s public structured model directly. They do not rely
on ANSI terminal rendering or other presentation-oriented output.

## MSRV

Rust 1.85 or newer.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
