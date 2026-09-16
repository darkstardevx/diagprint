# diagprint-axum 0.8.0 release checklist

Release date: 2026-09-16

## Release identity

- Package: `diagprint-axum`
- Version: `0.8.0`
- Branch: `axum-v0.8.0`
- MSRV: Rust 1.85
- Core dependency: `diagprint 0.7.1`
- Optional async dependency: `diagprint-async 0.7.0`
- Axum dependency: `0.8.9`
- Default features: none
- Optional features:
  - `async-delivery`

## Publication record

- Status: **published**
- Registry: `crates.io`
- Package: `diagprint-axum 0.8.0`
- Release date: `2026-09-16`
- Published source commit: `1044234e89b59056c5b313304cd28576a22ff1c7`
- Release tag: `diagprint-axum-v0.8.0`
- Core dependency: `diagprint 0.7.1`
- Optional async dependency: `diagprint-async 0.7.0`
- docs.rs status at post-release checkpoint: **available**
- docs.rs URL: `https://docs.rs/diagprint-axum/0.8.0/diagprint_axum/`

The crates.io artifact was downloaded after publication and independently
verified rather than relying only on the local package archive.

A fresh external consumer was also compiled against the registry artifact for
both the default feature surface and `async-delivery`.

Registry-sensitive release commands explicitly target `crates-io` so a locally
configured Cargo default registry cannot silently redirect release validation.

## Architecture boundary

`diagprint-axum` is a companion crate.

Dependency direction remains:

```text
Axum application
      |
      v
diagprint-axum
      |
      v
 diagprint core
```

The optional asynchronous path is:

```text
diagprint-axum
      |
      +---- async-delivery ----> diagprint-async
      |
      v
 diagprint core
```

`diagprint-async` must remain absent when `async-delivery` is disabled.

## Public release surface

The 0.8.0 release includes:

- privacy-safe `DiagnosticResponse`;
- explicit client disclosure policy;
- Axum rejection conversion;
- application-error adaptation;
- request ID validation and middleware;
- request/diagnostic correlation;
- RFC 9457 Problem Details;
- public and internal Problem Details extensions;
- explicit synchronous diagnostic emission;
- optional bounded async diagnostic delivery;
- server-side emission outcome metadata;
- feature-isolation guarantees.

## Privacy contract

By default, HTTP responses do not expose internal:

- diagnostic messages;
- diagnostic codes;
- source locations;
- notes;
- help;
- causes;
- arbitrary structured attributes;
- suggestions;
- remediation data;
- hostname;
- process ID;
- session ID.

Exposure remains explicit and policy-controlled.

Sink delivery and client disclosure are separate decisions.

## Synchronous emission contract

`DiagnosticEmissionExt::emit_to`:

- is explicit;
- performs one synchronous sink attempt;
- does not emit automatically;
- does not retry;
- does not queue;
- does not flush automatically;
- does not create background work;
- does not replace the HTTP response on sink failure.

## Async delivery contract

`AsyncDiagnosticEmissionExt::emit_to_async`:

- is available only with `async-delivery`;
- reuses `diagprint_async::AsyncDiagnosticSink`;
- uses the existing bounded queue;
- preserves configured backpressure behavior;
- does not create a second queue;
- does not create a worker per request;
- does not spawn one delivery task per diagnostic;
- does not retry automatically;
- does not flush automatically;
- does not own shutdown.

`AsyncEmissionOutcome::Enqueued` means queue acceptance, not completed sink
delivery.

## Required local gates

Before release metadata is considered final:

```bash
./scripts/diagprint-axum-gates quick
./scripts/diagprint-axum-gates full
./scripts/diagprint-axum-gates release
```

All must be green.

## Required workspace validation

```bash
./scripts/release-gates quick
./scripts/release-gates full
```

Both must be green on the exact release commit.

## Package validation

Required:

```bash
cargo package -p diagprint-axum
cargo publish --dry-run --registry crates-io -p diagprint-axum
```

## Registry prerequisites

Before real publication:

- `diagprint 0.7.1` must exist on crates.io;
- `diagprint-async 0.7.0` must exist on crates.io.

## Remote validation

The exact release commit must have green GitHub Actions results covering:

- formatting;
- whitespace;
- Clippy;
- default feature tests;
- all-feature tests;
- feature isolation;
- strict rustdoc;
- Rust 1.85 MSRV;
- package readiness.

## Tag preparation

No tag is created by the automated preparation scripts.

Before tagging:

```bash
git status --porcelain
git rev-parse HEAD
git tag --list --sort=-version:refname
```

Choose a tag naming convention consistent with existing repository history.

The tag must point to the exact release commit that passed local and remote
validation.

## Publication boundary

Real publication remains separate from validation.

Automated validation may run:

```bash
cargo publish --dry-run --registry crates-io -p diagprint-axum
```

A real publication command is deliberately not included here.

## Post-publication verification

Completed at the post-release checkpoint:

- [x] verify `diagprint-axum 0.8.0` is visible on crates.io;
- [x] download and inspect the exact crates.io package artifact;
- [x] verify the published manifest identity and Rust 1.85 MSRV;
- [x] verify the published `diagprint 0.7.1` dependency;
- [x] verify optional `diagprint-async 0.7.0`;
- [x] verify the default feature graph excludes `diagprint-async`;
- [x] verify `async-delivery` activates `diagprint-async`;
- [x] compile a fresh default-feature registry consumer;
- [x] compile a fresh `async-delivery` registry consumer;
- [x] verify the published README;
- [x] verify the published feature list;
- [x] verify the Git release tag points to the exact published source commit;
- [x] verify docs.rs documentation is available;
- [x] retain `[Unreleased]` for subsequent development.

The release tag remains fixed on the published source commit. Post-release
hardening commits intentionally occur after that tag.
