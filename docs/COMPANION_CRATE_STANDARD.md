# diagprint Companion Crate Standard

Every new diagprint companion crate must be created with its engineering,
documentation, testing, and release infrastructure from the beginning.

This is a project requirement.

## Purpose

Companion crates extend the diagprint diagnostics lifecycle into specific
frameworks, runtimes, protocols, or ecosystems without forcing those
dependencies into the core crate.

Examples include:

- diagprint-derive
- diagprint-lsp
- diagprint-async
- diagprint-otel
- diagprint-test
- diagprint-axum

The core crate should remain stable, reusable, and minimally coupled.

## Required at crate creation

Every new companion crate must include:

1. A package manifest with complete crates.io metadata.
2. A crate README.
3. Crate-level Rust documentation.
4. Meaningful tests for its public behavior and safety boundaries.
5. A dedicated gate script under the repository scripts directory.
6. Rust 1.85 MSRV verification unless the workspace MSRV changes deliberately.
7. Strict Clippy with warnings denied.
8. Crate-local strict rustdoc with warnings denied.
9. Workspace-wide strict rustdoc with all features enabled and warnings denied.
10. Package-content inspection.
11. A cargo package verification gate.
12. A cargo publish --dry-run release gate.
13. Registration in the root workspace release gates.
14. Explicit documentation of privacy, safety, trust, or failure boundaries
    where the integration crosses one.
15. Examples or usage documentation sufficient for a new user to understand
    the primary API without reading the source.

The rustdoc requirements are mandatory for every companion crate. Passing
crate-local documentation is not sufficient by itself: the crate must also
prove that its public documentation integrates cleanly with the complete
workspace when all features are enabled.

## Gate naming convention

Crate-specific gates live at:

    scripts/<crate-name>-gates

For example:

    scripts/diagprint-axum-gates quick
    scripts/diagprint-axum-gates full
    scripts/diagprint-axum-gates release

## Required gate modes

### quick

Used continuously during development.

It should include:

- unstaged whitespace validation;
- staged whitespace validation;
- formatting;
- crate check;
- strict crate Clippy;
- crate tests;
- architecture-specific invariants where appropriate.

### full

Used before a development checkpoint or push.

It must include everything in quick plus:

- doctests;
- crate-local strict rustdoc with warnings denied;
- workspace-wide strict rustdoc with all features enabled and warnings denied;
- package-content inspection;
- MSRV check;
- MSRV tests;
- MSRV rustdoc.

The required rustdoc commands are equivalent to:

    RUSTDOCFLAGS="-D warnings" \
    cargo doc \
      -p <crate-name> \
      --no-deps

and:

    RUSTDOCFLAGS="-D warnings" \
    cargo doc \
      --workspace \
      --no-deps \
      --all-features

The workspace-wide check exists to catch failures that a crate-local build can
miss, including ambiguous intra-doc links, cross-crate documentation issues,
and all-feature integration problems.

### release

Used immediately before publication.

It should include everything in full plus:

- clean-tree enforcement;
- cargo package verification;
- cargo publish --dry-run.

The gate script must never perform a real publication.

## Core isolation

Framework and ecosystem adapters must depend on diagprint.

diagprint core must not gain a dependency on a companion crate's framework or
runtime merely to support that integration.

The expected dependency direction is:

    external framework/runtime
              |
              v
      companion crate
              |
              v
         diagprint core

A new companion crate must not modify core behavior unless a separately
reviewed core bug fix or general-purpose lifecycle primitive is genuinely
required.

## Documentation standard

Each companion README should explain:

- what the crate does;
- what it deliberately does not do;
- installation;
- basic usage;
- primary public types or traits;
- safety/privacy behavior;
- failure behavior;
- MSRV;
- license;
- contributor gate commands.

Examples should favor complete, readable usage over cleverness.

Public Rust documentation is part of the compatibility contract. Every new
companion crate must keep both its own strict rustdoc build and the complete
workspace strict rustdoc build green before a checkpoint is considered done.

## Release discipline

A companion crate is not considered release-ready merely because cargo test
passes.

A release candidate should also demonstrate:

- strict lint cleanliness;
- crate-local strict rustdoc cleanliness;
- workspace-wide strict rustdoc cleanliness with all features enabled;
- fresh MSRV compatibility;
- correct package contents;
- successful cargo package;
- successful cargo publish --dry-run;
- green GitHub CI.

Publication remains deliberate and one crate at a time.
