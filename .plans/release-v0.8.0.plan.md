# Plan: diagprint v0.8.0 Release

Status: Approved

## Goal

Cut the completed `forensics-v0.8.0` train as an official synchronized v0.8.0
release before starting E2 SNAFU.

The release boundary includes:

```text
M4   causal diagnostic relationship graph + graph forensics
E1   reusable diagprint-bridge SDK + error-stack adapter
M5   remediation evidence + read-only replay/regression forensics
```

The release must publish a coherent crates.io package line, merge the completed
release train to `main`, tag the exact released commit, and create the GitHub
release before any E2 implementation begins.

## Release baseline

Last official release:

```text
tag: v0.7.1
main: c8362db755008ab98d8d834ce82dd5d60e6d696e
```

Published v0.7 artifacts:

```text
diagprint         0.7.1
diagprint-derive  0.7.0
diagprint-lsp     0.7.0
diagprint-async   0.7.0
diagprint-otel    0.7.0
diagprint-test    0.7.0
```

New packages introduced after v0.7.1:

```text
diagprint-bridge
diagprint-error-stack
```

Release candidate branch after M5 closure:

```text
forensics-v0.8.0
1409696f7e29714738a54b1509c77bf5ca641fce
```

Exact M5 closure CI:

```text
35188544365
success
```

## Version policy

Use one synchronized 0.8.0 package line.

Target versions:

```text
diagprint              0.8.0
diagprint-derive       0.8.0
diagprint-bridge       0.8.0
diagprint-error-stack  0.8.0
diagprint-lsp          0.8.0
diagprint-async        0.8.0
diagprint-otel         0.8.0
diagprint-test         0.8.0
```

Why synchronize all eight:

- `diagprint-bridge` and `diagprint-error-stack` are new public crates and should
  launch on the same product line as the core that defines their contracts.
- `diagprint-lsp`, `diagprint-async`, `diagprint-otel`, and `diagprint-test`
  currently require the 0.7 core line and therefore require new releases for
  clean 0.8 registry consumers.
- `diagprint-derive` is implementation-independent at the manifest level, but
  it generates code against diagprint's public API and is part of the same
  supported release family.
- one synchronized line makes docs, examples, support, release notes, and future
  compatibility policy substantially clearer.

Do not use mixed 0.7/0.8 public dependency requirements for this release.

## Dependency policy

After the version-alignment checkpoint:

```text
diagprint 0.8.0
  optional diagprint-derive = "0.8.0"

diagprint-bridge 0.8.0
  diagprint = "0.8.0"

diagprint-error-stack 0.8.0
  diagprint = "0.8.0"
  diagprint-bridge = "0.8.0"

diagprint-lsp 0.8.0
  diagprint = "0.8.0"

diagprint-async 0.8.0
  diagprint = "0.8.0"

diagprint-otel 0.8.0
  diagprint = "0.8.0"

diagprint-test 0.8.0
  diagprint = "0.8.0"
```

Path dependencies remain for workspace development while registry version
requirements describe the published dependency graph.

## Release train

### R8A — version alignment + release metadata

Change only release metadata/documentation/tooling required to prepare 0.8.0.

Expected work:

- bump all eight package versions to 0.8.0;
- align every internal registry dependency requirement to 0.8.0;
- regenerate/update Cargo.lock;
- curate the v0.8.0 changelog/release notes;
- update README installation examples/version references where applicable;
- update release tooling for dependency-aware staged publication;
- verify package file lists for all eight packages;
- run full workspace/MSRV gates;
- run all release checks that do not require unpublished 0.8 registry
  dependencies.

R8A must **not** require `cargo package` verification or
`cargo publish --dry-run` for a package whose 0.8 internal registry dependency
has not yet been published.

Cargo uses local `path` dependencies during workspace development, but the
`version` requirement is the registry fallback used for published packages.
Package/publish verification builds the packaged crate against registry
dependencies. Therefore a dependent 0.8 package cannot be fully verified
against crates.io until its required 0.8 dependency is visible there.

No feature implementation belongs in R8A.

Exact R8A CI must succeed before any registry publication.

### R8B — registry publication

Publish in dependency order.

Required order:

```text
1. diagprint-derive       0.8.0

2. diagprint              0.8.0

3. diagprint-bridge       0.8.0

4. diagprint-error-stack  0.8.0

5. diagprint-lsp          0.8.0
6. diagprint-async        0.8.0
7. diagprint-otel         0.8.0
8. diagprint-test         0.8.0
```

Steps 5-8 may be published in any order after `diagprint 0.8.0` is visible to
Cargo, but use the listed order for reproducibility.

Do not publish a dependent crate until its required 0.8 dependency is visible
from crates.io/Cargo registry resolution.

Each publish is irreversible. Before each real `cargo publish`:

- package version must equal 0.8.0;
- dependency requirements must equal the planned 0.8 line;
- every required internal 0.8 dependency must already be visible from the
  registry;
- `cargo package` must succeed;
- `cargo publish --dry-run` must succeed;
- working tree must be clean;
- exact release-preparation CI must be green.

After each real publish, verify the exact version is visible before continuing.

The staged verification/publish sequence is:

```text
derive:
  package
  publish --dry-run
  publish
  verify registry visibility

core:
  wait for derive 0.8.0 visibility
  package
  publish --dry-run
  publish
  verify registry visibility

bridge:
  wait for diagprint 0.8.0 visibility
  package
  publish --dry-run
  publish
  verify registry visibility

error-stack:
  wait for diagprint-bridge 0.8.0 visibility
  package
  publish --dry-run
  publish
  verify registry visibility

lsp / async / otel / test:
  wait for diagprint 0.8.0 visibility
  package
  publish --dry-run
  publish
  verify registry visibility
```

Do not use `--no-verify` as a substitute for dependency-aware sequencing.

### R8C — main merge, tag, GitHub release

After all eight crates are confirmed published:

1. merge the exact release-preparation commit/history to `main`;
2. verify `main` contains the complete M4 + E1 + M5 train;
3. verify CI on the exact merged `main` commit;
4. create annotated tag:

```text
v0.8.0
```

pointing to the exact released `main` commit;

5. push the tag;
6. create GitHub release `diagprint v0.8.0`;
7. include the exact package matrix and release highlights;
8. verify tag and release point to the same commit.

Do not tag an unmerged feature-branch commit if `main` is intended to remain the
canonical released source.

## Release highlights

The v0.8.0 notes should prominently cover:

### Causal diagnostic graph / forensics

- canonical diagnostic relationship graph;
- evidence-aware relationship kinds;
- deterministic graph digest/verification;
- graph traversal;
- `diagprint graph`;
- Git/run provenance binding;
- `diagprint blame`;
- doctrine that provenance/association does not imply causation.

### Ecosystem interoperability

- core `InteropDiagnostic` protocol;
- new `diagprint-bridge` reusable adapter SDK;
- builder-scoped ephemeral construction handles;
- canonical fingerprint graph identity;
- new `diagprint-error-stack` adapter;
- single and grouped `error-stack` reports;
- structured source topology;
- attachment privacy defaults;
- no rendered-report parsing.

### Remediation evidence / replay

- `diagprint.remediation.evidence/v1`;
- exact receipt-to-history transition binding;
- append-only privacy-light evidence sidecars;
- `diagprint.forensics.remediation-replay/v1`;
- `diagprint replay`;
- `diagprint history remediation-verify`;
- verified resolution/reappearance lifecycle evidence;
- explicit causal boundaries;
- read-only replay.

## Release validation

### Before any registry publication

Require:

```bash
./scripts/release-gates full
```

Also inspect package file lists for all eight packages:

```bash
cargo package --allow-dirty --list -p diagprint-derive
cargo package --allow-dirty --list -p diagprint
cargo package --allow-dirty --list -p diagprint-bridge
cargo package --allow-dirty --list -p diagprint-error-stack
cargo package --allow-dirty --list -p diagprint-lsp
cargo package --allow-dirty --list -p diagprint-async
cargo package --allow-dirty --list -p diagprint-otel
cargo package --allow-dirty --list -p diagprint-test
```

These prepublication checks validate workspace quality and package contents
without pretending that unpublished 0.8 registry dependencies already exist.

### During staged registry publication

Run full package verification and publish dry-run immediately before each real
publish, after that package's internal 0.8 registry dependencies are visible.

Required staged gates:

```text
diagprint-derive
  cargo package
  cargo publish --dry-run

diagprint
  after diagprint-derive 0.8.0 is visible
  cargo package
  cargo publish --dry-run

diagprint-bridge
  after diagprint 0.8.0 is visible
  cargo package
  cargo publish --dry-run

diagprint-error-stack
  after diagprint-bridge 0.8.0 is visible
  cargo package
  cargo publish --dry-run

diagprint-lsp / async / otel / test
  after diagprint 0.8.0 is visible
  cargo package
  cargo publish --dry-run
```

Release tooling may expose dedicated staged modes for these checks, but the
dependency ordering above is authoritative.

A successful local workspace build does not prove registry resolution because
local `path` dependencies are used during development while their `version`
requirements become registry dependencies in the published package.

## Registry verification

Because crates.io publication is immutable for a package/version pair:

- never retry a successful publish blindly;
- after an ambiguous network failure, check registry visibility first;
- if 0.8.0 is visible, treat it as published and continue;
- if it is not visible, inspect the actual Cargo error before retrying;
- never change package contents and attempt to overwrite the same published
  version.

Registry verification should check exact package/version identity, not only a
search result title.

## Git / branch policy

Current release work remains on:

```text
forensics-v0.8.0
```

until the release-preparation checkpoint is green.

No E2 SNAFU implementation is allowed on this branch before v0.8.0 completion.

After publication:

- merge the exact release train to `main`;
- tag the exact released `main` commit;
- preserve the feature branch as historical development evidence unless there
  is a separate cleanup decision.

## main synchronization

At plan creation, `main` is still at v0.7.1:

```text
c8362db755008ab98d8d834ce82dd5d60e6d696e
```

If `main` moves before R8C:

- stop;
- fetch;
- inspect divergence;
- reconcile explicitly;
- rerun release validation;
- do not force-push or blindly overwrite main.

## No feature creep

The release train may change:

- package versions;
- internal package version requirements;
- Cargo.lock;
- release notes/changelog;
- installation/version documentation;
- release scripts when necessary for safe deterministic publication.

The release train must not add:

- SNAFU;
- new adapters;
- new schemas;
- new runtime behavior;
- new CLI commands;
- unrelated refactors.

If a release-blocking defect requires code changes, record the defect in this
plan, fix it in a dedicated release-fix checkpoint, and require exact green CI
again before publishing.

## crates.io package-name check

Before first publication of the two new packages, explicitly verify ownership /
availability for:

```text
diagprint-bridge
diagprint-error-stack
```

Do not assume a successful local package dry run reserves a crates.io name.

If either name is unavailable to the publisher, stop and revise the release
plan before renaming anything.

## Documentation requirements

Release preparation should produce a curated v0.8.0 section describing:

```text
v0.7.1 -> v0.8.0
```

Include:

- compatibility/MSRV;
- all eight published package versions;
- two new crates;
- new public schemas;
- new CLI commands;
- privacy behavior;
- causal evidence doctrine;
- upgrade examples;
- no-default-feature posture.

## MSRV

Keep:

```text
Rust 1.85
edition 2024
```

Any dependency resolution that breaks Rust 1.85 is a release blocker.

## E2 gate

E2 SNAFU stays roadmap-next but unimplemented until all of the following are
true:

```text
all eight v0.8.0 crates published
main contains released v0.8.0 tree
main CI green
v0.8.0 tag pushed
GitHub v0.8.0 release created
release plan closed
```

Only then open a new E2 Draft -> Approved -> committed plan.

## Expected R8A file boundary

Expected modified files:

```text
Cargo.toml
Cargo.lock

crates/diagprint-derive/Cargo.toml
crates/diagprint-bridge/Cargo.toml
crates/diagprint-error-stack/Cargo.toml
crates/diagprint-lsp/Cargo.toml
crates/diagprint-async/Cargo.toml
crates/diagprint-otel/Cargo.toml
crates/diagprint-test/Cargo.toml

README.md
CHANGELOG.md
```

Possible only if needed:

```text
scripts/release-gates
docs/*
```

Protected during pure release preparation:

```text
src/
tests/
crates/*/src/
crates/*/tests/
crates/*/examples/
.github/workflows/
```

Any need to change protected implementation paths requires an explicit plan
revision or dedicated release-fix checkpoint.

## Acceptance criteria

The v0.8.0 release is complete when:

- M5 closure CI is recorded green;
- all eight manifests declare version 0.8.0;
- internal public dependency requirements are aligned to 0.8.0;
- Cargo.lock is consistent;
- Rust 1.85 is green;
- full prepublication workspace/MSRV gates pass;
- package file-list inspection passes for all eight packages;
- each package verification and publish dry-run passes at its dependency-aware
  publication stage;
- `diagprint-bridge` 0.8.0 is published;
- `diagprint-error-stack` 0.8.0 is published;
- all six existing package families have their planned 0.8.0 publication;
- exact registry visibility is confirmed for all eight;
- the release train is merged to `main`;
- exact merged-main CI is green;
- tag `v0.8.0` points to that exact main commit;
- the GitHub v0.8.0 release exists and points to the same tag;
- release notes list the exact package matrix;
- the release plan is closed;
- E2 remains unimplemented until closure.

## Completion record

```text
M5 closure commit: 1409696f7e29714738a54b1509c77bf5ca641fce
M5 closure CI run: 35188544365
M5 closure CI result: success

Release-sequencing revision commit: e51e3289e914e6c933ff55ba66430d3233b44e46
Release-sequencing revision CI run: 35191143109
Release-sequencing revision CI result: success

R8A release-preparation commit: fe443b31bfbd9e5d32cdbd6f127f8a751507f3c6
R8A CI run: 35266729806
R8A CI result: failure — package-readiness workflow used a stale hard-coded archive lookup after successfully building diagprint-derive-0.8.0.crate; format/clippy, tests, MSRV, package-content inspection, generated-state exclusion, and first-stage archive build passed.

R8A release-fix commit:
R8A release-fix CI run:
R8A release-fix CI result:

Published:
diagprint-derive 0.8.0:
diagprint 0.8.0:
diagprint-bridge 0.8.0:
diagprint-error-stack 0.8.0:
diagprint-lsp 0.8.0:
diagprint-async 0.8.0:
diagprint-otel 0.8.0:
diagprint-test 0.8.0:

Merged main commit:
Merged main CI run:
Merged main CI result:

Tag:
GitHub release:

Notes:
- R8A release-fix derives both `diagprint-derive` version and Cargo `target_directory` from `cargo metadata`; local Cargo uses `~/.cargo-target`, while GitHub Actions may use a different target directory.
```
