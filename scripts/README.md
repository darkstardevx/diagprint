# diagprint Gate Scripts

diagprint uses repository-owned gate scripts so maintainers and contributors
can run the same validation locally that is expected before a release.

## Workspace gates

The workspace release runner is:

    ./scripts/release-gates quick
    ./scripts/release-gates full
    ./scripts/release-gates core
    ./scripts/release-gates satellites

The release runner performs validation and publish dry-runs only.

It never performs an actual crates.io publication.

## Companion crate gates

Every companion crate receives a dedicated gate script when the crate is
created.

The convention is:

    scripts/<crate-name>-gates

The usual modes are:

    quick
    full
    release

For example:

    ./scripts/diagprint-axum-gates quick

runs the fast development contract.

    ./scripts/diagprint-axum-gates full

runs tests, documentation, packaging inspection, and Rust 1.85 validation.

    ./scripts/diagprint-axum-gates release

adds clean-tree enforcement, cargo package verification, and cargo publish
--dry-run.

## Why crate-specific gates exist

A contributor working only on one integration should not need to understand
the entire release train before they can validate their work.

Crate-local gates also make important integration-specific invariants
executable.

For example, diagprint-axum verifies that:

- diagprint core remains on the 0.7.x line;
- Axum remains outside core;
- crate tests are green;
- Rust 1.85 still works;
- the package is independently publishable.

See:

    docs/COMPANION_CRATE_STANDARD.md
