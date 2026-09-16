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
