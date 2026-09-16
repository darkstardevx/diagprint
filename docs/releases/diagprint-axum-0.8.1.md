# diagprint-axum 0.8.1 publication record

Release date: 2026-09-16

## Release identity

- Status: **published**
- Registry: `crates.io`
- Package: `diagprint-axum 0.8.1`
- Published source commit: `530d267c82cceff3b81e859fdc8abc364892fab1`
- Release tag: `diagprint-axum-v0.8.1`
- Pre-release CI run: `35144494534`
- Published artifact SHA-256: `74eecfa6a9f499b2843203197ec29037000ee707980414d906d0b6d74e24571a`
- MSRV: Rust 1.85
- Core dependency: `diagprint 0.7.1`
- Optional async dependency: `diagprint-async 0.7.0`
- Axum dependency line: `0.8.9`
- Default features: none
- Optional feature: `async-delivery`

The release tag points to the exact source revision that passed the final
security and reproducibility audit.

This publication record intentionally follows the immutable release tag.

## Artifact verification

The published archive was downloaded directly from the crates.io static CDN:

`https://static.crates.io/crates/diagprint-axum/diagprint-axum-0.8.1.crate`

Published SHA-256:

`74eecfa6a9f499b2843203197ec29037000ee707980414d906d0b6d74e24571a`

This exactly matches the audited pre-publication artifact SHA-256:

`74eecfa6a9f499b2843203197ec29037000ee707980414d906d0b6d74e24571a`

The registry artifact therefore matches the exact package bytes validated
before publication.

The published archive was independently checked for:

- package name and version;
- Rust 1.85 MSRV;
- empty default feature set;
- `async-delivery` feature mapping;
- registry-only `diagprint 0.7.1` dependency;
- optional registry-only `diagprint-async 0.7.0` dependency;
- exact Cargo VCS source SHA;
- application-state and Result-adapter modules;
- packaged synchronous and asynchronous examples;
- adversarial security-contract tests.

## Downstream verification

A fresh registry-only default-feature application compiled successfully against
`diagprint-axum = "=0.8.1"`.

Its dependency graph excludes `diagprint-async`.

A second fresh registry-only application compiled successfully with
`async-delivery`.

Its dependency graph activates `diagprint-async 0.7.0`.

The common asynchronous construction and outcome types remain available
through `diagprint-axum` without requiring a direct `diagprint-async`
dependency for the common Axum setup.

## Security contract

The published release retains the fail-closed HTTP externalization boundary.

By default, client responses do not expose internal diagnostic messages,
codes, structured attributes, notes, help, causes, labels, source paths,
process metadata, host metadata, session metadata, or internal Problem Details
extensions.

Invalid inbound request IDs are replaced rather than reflected.

Problem Details responses use:

- `application/problem+json`;
- `Cache-Control: no-store`.

## Async contract

`AsyncEmissionOutcome::Enqueued` means queue acceptance only, not completed
delivery or persistence.

The Axum adapter reuses the existing bounded async queue, preserves configured
backpressure, creates no second queue or per-request worker, performs no hidden
retry, does not flush automatically, and does not own shutdown.

## Immutability

Immutable release source:

`530d267c82cceff3b81e859fdc8abc364892fab1`

Immutable release tag:

`diagprint-axum-v0.8.1`

Published crates.io version `0.8.1` must never be republished.

Post-release commits must never move the release tag.
