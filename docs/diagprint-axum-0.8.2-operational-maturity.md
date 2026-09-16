# diagprint-axum 0.8.2 operational maturity

The 0.8.2 development line begins from the post-release state of
`diagprint-axum 0.8.1`.

The published 0.8.1 release remains immutable:

- release source:
  `530d267c82cceff3b81e859fdc8abc364892fab1`;
- release tag:
  `diagprint-axum-v0.8.1`;
- published artifact SHA-256:
  `74eecfa6a9f499b2843203197ec29037000ee707980414d906d0b6d74e24571a`.

0.8.2 is an operational-maturity line. New public API should not be added
merely for convenience.

## SemVer contract

Every 0.8.2 API change is checked as a patch release against the published
`diagprint-axum 0.8.1` registry baseline.

The automated SemVer check uses all features.

`cargo-semver-checks` is an important guard, not the sole compatibility proof.
Real downstream compilation, packaged examples, behavioral tests, and release
artifact verification remain necessary because static SemVer tooling cannot
detect every possible behavioral compatibility break.

## Property testing

Property-based tests exercise invariants rather than only known examples.

Current properties cover:

- `RequestId` acceptance exactly matching its documented grammar and maximum
  length;
- Problem Details extension-name acceptance matching the documented portable
  naming and reserved-name rules;
- arbitrary generated diagnostic secrets remaining absent from the default
  Problem Details representation.

Any minimized property-test regression becomes a concrete regression case.

## Coverage-guided fuzzing

The development-only fuzz workspace is intentionally excluded from the
published crate.

Current libFuzzer targets cover:

- arbitrary request-ID input;
- arbitrary Problem Details extension names.

The fuzz workflow runs on Linux because the libFuzzer sanitizer integration is
Linux-oriented.

Fuzzing is scheduled weekly and is also manually dispatchable.

A fuzz crash is release-blocking until it is understood, minimized, and turned
into a durable regression test where practical.

## Compatibility matrix

The operational CI matrix covers:

- current stable Rust on Linux;
- current stable Rust on macOS;
- current stable Rust on Windows;
- Rust 1.85 MSRV on Linux;
- the no-default-feature surface;
- the `async-delivery` surface;
- the complete feature powerset.

The existing workspace CI remains authoritative for integration with the rest
of diagprint.

## Async invariants

Operational changes must preserve the 0.8.1 delivery contract:

- `Enqueued` means queue acceptance, not completed delivery;
- `Reject` remains explicit backpressure failure;
- `DropNewest` remains an explicit deliberate drop;
- Axum creates no second queue;
- Axum creates no per-request delivery worker;
- no automatic retry is added;
- no automatic flush is added;
- application lifecycle retains shutdown ownership.

## Privacy invariants

Operational changes must preserve the fail-closed HTTP externalization
boundary.

Private diagnostics must not become public merely because of refactoring,
feature combinations, platform differences, serialization changes, or async
delivery.

## Longer-term hardening

Subsequent 0.8.2 milestones should concentrate on operational assurance rather
than API expansion.

Candidate work includes:

- persistent fuzz corpora for discovered edge cases;
- longer scheduled fuzz campaigns;
- dependency and license policy automation;
- supply-chain vulnerability scanning;
- failure-injection tests for sinks and async worker termination;
- deterministic concurrency tests in `diagprint-async` where queue semantics
  warrant them;
- public API snapshot review in addition to SemVer linting;
- compatibility checks when a newer Axum 0.8.x release becomes available;
- load and backpressure characterization for representative applications;
- documentation tests that compile realistic downstream application patterns.

## Release principle

No 0.8.2 release should weaken the security, privacy, async-delivery, MSRV,
packaging, or reproducibility guarantees established for 0.8.1.

## Milestone status

### Milestone 1 — operational foundation — closed

Milestone 1 is closed at:

`a2ceff1694d3c04b1184e308cfe714a07ed1b34f`

The checkpoint established:

- patch-SemVer automation against published `0.8.1`;
- property-based request-ID and Problem Details validation;
- generated-secret privacy properties;
- libFuzzer harnesses;
- weekly fuzz CI;
- stable Linux, macOS, and Windows compatibility;
- Rust 1.85 MSRV validation;
- complete Axum feature-powerset checking;
- package exclusion of development-only fuzz state;
- a reusable operational gate.

The checkpoint passed both the repository CI and the dedicated Axum
Operational Maturity workflow before Milestone 2 began.

### Milestone 2 — failure injection and async concurrency

Milestone 2 deliberately attacks delivery and lifecycle failure paths without
changing the runtime API.

Through the public `diagprint-async` API from the `diagprint-axum` operational test suite it verifies:

- an underlying `DiagnosticSink::emit` failure becomes a sticky
  `AsyncSinkError::Worker`;
- a `DiagnosticSink::flush` failure becomes a sticky worker failure;
- later submissions fail after the worker has failed;
- shutdown preserves the underlying worker failure;
- a panicking blocking worker closes future submission;
- worker panic is reported as a join failure at lifecycle shutdown;
- queue acceptance remains distinct from completed sink delivery.

At the `diagprint-axum` layer it verifies:

- an HTTP response already accepted for queue submission is not replaced when
  underlying delivery later fails;
- later requests retain their original HTTP response even when the async worker
  is already failed;
- worker error text remains server-side and never enters the client body;
- concurrent `Reject` saturation reports `QueueFull` only as response metadata;
- rejected diagnostics never reach the underlying sink;
- concurrent `DropNewest` saturation reports every deliberate drop;
- dropped diagnostics never reach the underlying sink;
- concurrent `Block` submissions remain pending while capacity is unavailable;
- releasing capacity allows every blocked submission to complete;
- every accepted diagnostic reaches the underlying sink exactly once;
- per-request correlation remains intact under concurrent pressure;
- raw query secrets, internal diagnostic messages, codes, attributes, and
  operational failure details stay outside client responses.

These tests use one real `AsyncDiagnosticSink` queue and worker. They do not
introduce a test-only queueing abstraction that could hide production
behavior.

The concurrency contract is intentionally repeated in operational CI to catch
timing-sensitive regressions while still using deterministic queue saturation
and explicit release gates rather than probabilistic sleeps as the primary
synchronization mechanism.

Milestone 2 remains test- and operations-only unless this fault injection
exposes a genuine implementation defect. A failing test is not repaired by
weakening the assertion unless the documented public contract itself is
intentionally changed.



The worker lifecycle failure tests live under `diagprint-axum/tests` rather
than modifying `diagprint-async`. They exercise the frozen companion crate
through its existing public API while preserving the Axum branch-isolation
contract. Emit failure, flush failure, worker panic, sticky error state, queue
closure, and shutdown/join behavior remain covered.

### Milestone 3 — supply chain and API provenance

Milestone 3 hardens the dependency, build-workflow, and public-API trust
boundary without expanding runtime behavior.

The dependency policy uses `cargo-deny` against both the default and
`async-delivery` `diagprint-axum` graphs. It checks advisories, yanked and
unsound packages, licenses, wildcard dependency declarations, and package
sources.

A second RustSec implementation, `cargo-audit`, independently scans a fresh
standalone consumer lockfile for the async-delivery graph.

Canonical dependency provenance snapshots record exact package versions,
sources, registry checksums, and activated features for both the default and
async-delivery graphs. Any graph drift therefore becomes visible in review.

Public API provenance uses `cargo-public-api 0.52.0` with
`nightly-2025-11-22`.

That dated nightly is intentional. Rustdoc JSON format 57 introduced the
`ExternalCrate::path` field required by the parser used by the pinned
cargo-public-api installation. Earlier format-55 JSON from
`nightly-2025-08-02` is rejected because that field is absent.

The current default and all-feature API snapshots are committed and compared
against both current source and the immutable `diagprint-axum-v0.8.1` release
tag.

For the 0.8.2 operational-maturity line, public API equality with 0.8.1 is
intentionally stricter than ordinary patch SemVer compatibility: even
compatible API additions require a deliberate policy change.

Every external GitHub Action is pinned to a reviewed full-length commit SHA.
The policy also requires explicit workflow permissions, disables persisted
checkout credentials, rejects `pull_request_target`, and rejects `write-all`.

Dependabot discovers candidate Cargo and GitHub Actions updates, but discovery
does not bypass provenance review.

The dedicated Axum Supply Chain workflow runs daily so newly published
advisories can fail the security gate even when repository source and the
lockfile have not changed.


The dated public-API toolchain is pinned to rustdoc JSON format 57 or later
because that schema introduced `ExternalCrate::path`, which is required by the
pinned parser. This prevents the provenance check from silently depending on a
moving nightly while also avoiding the incompatible format-55 schema.

Milestone 3 changes no `diagprint-axum` or `diagprint-async` runtime source.
