# diagprint Companion Crate Workflow

This guide is the practical companion to `docs/COMPANION_CRATE_STANDARD.md`.

It describes the expected workflow for creating, validating, documenting, checkpointing, and preparing a new diagprint companion crate for release. The goal is to make the process easy to follow for maintainers and contributors while preserving the same quality bar across every crate.

The standard applies to companion crates such as:

- `diagprint-derive`
- `diagprint-lsp`
- `diagprint-async`
- `diagprint-otel`
- `diagprint-test`
- `diagprint-axum`
- future framework, runtime, protocol, or ecosystem adapters

## Core rule

A companion crate extends diagprint without pulling framework-specific dependencies back into core.

The expected dependency direction is:

```text
external framework/runtime
          |
          v
  companion crate
          |
          v
     diagprint core
```

For the v0.8-era work, core `diagprint` remains on the stable v0.7.x line except for fixes. Companion crates may advance independently when that is useful.

## 1. Create the branch

Start from an up-to-date `main` branch.

```bash
git switch main
git pull --ff-only origin main
git switch -c <crate-branch>
```

### What these commands do

- `git switch main` moves the working tree to the main branch.
- `git pull --ff-only origin main` updates local `main` only when Git can perform a fast-forward. It refuses to create an accidental merge commit.
- `git switch -c <crate-branch>` creates and checks out the development branch for the new crate.

### Help

- Git switch: https://git-scm.com/docs/git-switch
- Git pull: https://git-scm.com/docs/git-pull

## 2. Add the crate to the workspace

Create the crate under `crates/` and register it in the root workspace.

Typical layout:

```text
crates/<crate-name>/
├── Cargo.toml
├── README.md
├── src/
│   └── lib.rs
└── tests/
```

The root `Cargo.toml` must list the crate as a workspace member.

### Why this matters

Workspace membership allows Cargo to build, test, lint, document, and package the crate consistently with the rest of diagprint.

### Help

- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo manifest format: https://doc.rust-lang.org/cargo/reference/manifest.html

## 3. Add complete package metadata immediately

A new companion crate should begin with release-quality metadata instead of adding it at the end.

At minimum, define:

- `name`
- `version`
- `edition`
- `rust-version`
- `description`
- `license`
- `repository`
- `homepage`
- `documentation`
- `readme`
- useful `keywords`
- useful `categories`

The crate should depend on the stable diagprint core release line rather than requiring unpublished core changes unless a separately reviewed general-purpose fix is genuinely necessary.

### Help

- Cargo package metadata: https://doc.rust-lang.org/cargo/reference/manifest.html#the-package-section
- Cargo SemVer compatibility: https://doc.rust-lang.org/cargo/reference/semver.html

## 4. Create the README and crate-level Rust documentation

Every companion crate must explain:

- what it does;
- what it deliberately does not do;
- installation;
- a basic example;
- the primary public API;
- privacy, safety, trust, or failure boundaries;
- MSRV;
- license;
- contributor gate commands.

The README should let a new user understand the crate without reading the implementation first.

Crate-level Rust documentation belongs at the top of `src/lib.rs` using `//!` comments.

### Important rustdoc rule

Public documentation is part of the compatibility contract.

Every companion crate must pass both:

1. crate-local strict rustdoc; and
2. workspace-wide strict rustdoc with all features enabled.

The second check catches failures that a crate-only documentation build can miss, including ambiguous intra-doc links and cross-crate all-feature interactions.

### Help

- rustdoc book: https://doc.rust-lang.org/rustdoc/
- rustdoc lints: https://doc.rust-lang.org/rustdoc/lints.html
- intra-doc links: https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html

## 5. Add meaningful tests before expanding the API

Tests should cover public behavior and the integration's safety boundaries.

Prefer tests that exercise the real framework or protocol where practical instead of only testing helper functions in isolation.

Examples include:

- response status preservation;
- privacy/redaction defaults;
- structured metadata preservation;
- stale-state handling;
- failure classification;
- public wrapper ergonomics;
- size or allocation regressions when they matter to common API paths.

### Help

- Cargo test: https://doc.rust-lang.org/cargo/commands/cargo-test.html
- Rust testing chapter: https://doc.rust-lang.org/book/ch11-00-testing.html

## 6. Create the crate-specific gate script immediately

Every new companion crate must receive a dedicated script when the crate is created.

Naming convention:

```text
scripts/<crate-name>-gates
```

Example:

```text
scripts/diagprint-axum-gates
```

The script must support these modes:

```bash
./scripts/<crate-name>-gates quick
./scripts/<crate-name>-gates full
./scripts/<crate-name>-gates release
```

The script may stop on failure internally. It must not require the user to enable shell-wide `set -e` or similar options in an interactive terminal.

## 7. Quick gate

Use `quick` continuously during development.

Typical command:

```bash
./scripts/<crate-name>-gates quick
```

The quick gate should include the following checks.

### `git diff --check`

```bash
git diff --check
```

Checks unstaged changes for whitespace errors such as trailing spaces and malformed conflict markers.

Help: https://git-scm.com/docs/git-diff

### `git diff --cached --check`

```bash
git diff --cached --check
```

Performs the same whitespace validation against staged changes.

This matters because staged content can differ from the working tree.

### `cargo fmt --all -- --check`

```bash
cargo fmt --all -- --check
```

Checks formatting across the workspace without rewriting files.

During active development, use this to apply formatting first:

```bash
cargo fmt --all
```

Help: https://github.com/rust-lang/rustfmt

### `cargo check`

```bash
cargo check \
  -p <crate-name> \
  --all-targets
```

Type-checks the crate and all selected targets without producing final binaries. It is usually faster than a full build and catches compiler errors early.

Help: https://doc.rust-lang.org/cargo/commands/cargo-check.html

### Strict Clippy

```bash
cargo clippy \
  -p <crate-name> \
  --all-targets \
  -- \
  -D warnings
```

Runs Clippy and upgrades warnings to hard failures.

The project does not weaken this gate to make a warning disappear. Fix the issue unless there is a narrowly justified lint exception.

Help: https://doc.rust-lang.org/clippy/

### Crate tests

```bash
cargo test \
  -p <crate-name> \
  --all-targets
```

Runs the crate's tests across all selected targets.

Help: https://doc.rust-lang.org/cargo/commands/cargo-test.html

### Architecture-specific invariants

Each crate should add checks specific to its boundary.

For example, `diagprint-axum` verifies that:

- core remains on the 0.7.x line;
- the Axum adapter does not modify frozen core source;
- unrelated companion crate source is not changed accidentally.

These checks should be executable, not merely documented promises.

## 8. Full gate

Run `full` before a checkpoint or push that is meant to represent completed work.

```bash
./scripts/<crate-name>-gates full
```

The full gate includes everything in `quick` plus the following checks.

### Doctests

```bash
cargo test \
  -p <crate-name> \
  --doc
```

Compiles and runs Rust examples embedded in documentation where applicable.

This protects examples from silently becoming stale.

Help: https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html

### Crate-local strict rustdoc

```bash
RUSTDOCFLAGS="-D warnings" \
cargo doc \
  -p <crate-name> \
  --no-deps
```

Builds the crate's public documentation and turns rustdoc warnings into errors.

`--no-deps` keeps the check focused on project-owned documentation rather than rebuilding dependency documentation.

Help: https://doc.rust-lang.org/cargo/commands/cargo-doc.html

### Workspace-wide strict rustdoc with all features

```bash
RUSTDOCFLAGS="-D warnings" \
cargo doc \
  --workspace \
  --no-deps \
  --all-features
```

This is mandatory for every companion crate.

It verifies that the new crate does not break documentation when integrated with the complete workspace and every feature combination enabled by `--all-features`.

This gate was added after an early `diagprint-axum` CI run exposed an ambiguous intra-doc link that was not obvious from the narrower crate-only workflow.

Help:

- Cargo doc: https://doc.rust-lang.org/cargo/commands/cargo-doc.html
- Cargo features: https://doc.rust-lang.org/cargo/reference/features.html
- rustdoc intra-doc links: https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html

### Package-content inspection

```bash
cargo package \
  --allow-dirty \
  --list \
  -p <crate-name>
```

Shows exactly which files Cargo intends to put in the published `.crate` archive.

Use this to verify that required files such as `README.md` and `src/lib.rs` are present and that generated or private development state is not accidentally included.

Help: https://doc.rust-lang.org/cargo/commands/cargo-package.html

## 9. MSRV validation

The current diagprint workspace MSRV is Rust 1.85 unless intentionally changed.

Install the toolchain if necessary:

```bash
rustup toolchain install 1.85.0
```

Check installed toolchains:

```bash
rustup toolchain list
```

Help: https://rust-lang.github.io/rustup/concepts/toolchains.html

### MSRV check

```bash
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback \
cargo +1.85.0 check \
  -p <crate-name> \
  --all-targets
```

Verifies that the crate compiles on the declared minimum Rust version.

The resolver environment variable tells Cargo to prefer dependency versions compatible with the active Rust version when possible.

### MSRV tests

```bash
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback \
cargo +1.85.0 test \
  -p <crate-name> \
  --all-targets
```

Runs the crate's tests using the declared minimum Rust toolchain.

### MSRV rustdoc

```bash
RUSTDOCFLAGS="-D warnings" \
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback \
cargo +1.85.0 doc \
  -p <crate-name> \
  --no-deps
```

Verifies that public documentation also builds cleanly on the declared MSRV.

## 10. Register the crate with workspace release gates

A new companion crate must be added to the root release-gate machinery so workspace release checks cannot forget it.

The root runner is:

```bash
./scripts/release-gates quick
./scripts/release-gates full
./scripts/release-gates core
./scripts/release-gates satellites
```

A crate may also receive a dedicated root mode when useful, as `diagprint-axum` does.

The root release runner performs validation and dry-runs only. It must never perform a real publication.

## 11. Run the workspace quick gate after changing shared release infrastructure

If the new crate changes workspace membership, `Cargo.lock`, shared scripts, CI configuration, or root release logic, run:

```bash
./scripts/release-gates quick
```

This confirms that the integration did not break the wider repository.

## 12. Inspect the exact change boundary

Before checkpointing, inspect what changed.

```bash
git diff --check
git status --short
git diff --stat
```

Useful focused diff:

```bash
git diff -- \
  Cargo.toml \
  Cargo.lock \
  scripts \
  docs \
  crates/<crate-name>
```

### What these commands do

- `git diff --check` catches whitespace errors.
- `git status --short` gives a compact changed/staged/untracked file list.
- `git diff --stat` shows the size and distribution of the change.
- `git diff -- <paths>` narrows review to the expected architectural boundary.

Help: https://git-scm.com/docs/git-diff

## 13. Checkpoint completed work

A checkpoint should represent a coherent, green unit of work.

Typical sequence:

```bash
git add <expected paths>
git commit
git push origin <branch>
```

The commit message should explain:

- behavior added or changed;
- safety/privacy implications;
- tests added;
- documentation added;
- gates run;
- architecture boundaries preserved.

Do not hide unrelated changes in the checkpoint.

### Help

- Git add: https://git-scm.com/docs/git-add
- Git commit: https://git-scm.com/docs/git-commit
- Git push: https://git-scm.com/docs/git-push

## 14. Remote GitHub CI is part of completion

A local green gate is necessary but not sufficient.

The checkpoint is not considered fully validated until the corresponding GitHub Actions run succeeds.

The diagprint CI currently validates areas including:

- formatting and whitespace;
- Clippy;
- default-feature checks and tests;
- all-feature checks and tests;
- doctests and strict rustdoc;
- feature-isolation checks;
- Rust 1.85 MSRV compilation, tests, and docs;
- package readiness.

Use the repository helper when available:

```bash
gh-ci-watch CI <branch-or-ref>
```

Or use GitHub CLI directly:

```bash
gh run list
gh run watch <run-id>
```

### Help

- GitHub Actions workflows: https://docs.github.com/en/actions/concepts/workflows-and-actions/workflows
- GitHub Actions reference: https://docs.github.com/en/actions/reference
- GitHub CLI `gh run`: https://cli.github.com/manual/gh_run

## 15. Release gate

Run the crate-specific release gate only when the crate is approaching publication.

```bash
./scripts/<crate-name>-gates release
```

The release gate includes everything in `full` plus clean-tree and packaging checks.

### Clean-tree enforcement

The release gate should refuse to continue when tracked or untracked release-relevant changes remain.

Useful manual check:

```bash
git status --porcelain
```

An empty result means the working tree is clean.

### `cargo package`

```bash
cargo package \
  -p <crate-name>
```

Creates and verifies the publishable `.crate` archive locally.

By default Cargo verifies the packaged crate by building it.

Help: https://doc.rust-lang.org/cargo/commands/cargo-package.html

### `cargo publish --dry-run`

```bash
cargo publish \
  --dry-run \
  -p <crate-name>
```

Runs Cargo's publication checks without uploading anything to crates.io.

The gate script must stop here.

A real `cargo publish` command is intentionally never embedded in automated gate scripts.

Help: https://doc.rust-lang.org/cargo/commands/cargo-publish.html

## 16. Real publication is a separate irreversible action

Publishing to crates.io is deliberately manual and separate from validation.

Before any real publication, confirm all of the following:

- crate-specific `release` gate is green;
- workspace release gates relevant to the crate are green;
- GitHub CI is green on the exact commit being published;
- package contents have been inspected;
- version and dependency requirements are correct;
- changelog/release documentation is ready;
- the working tree is clean;
- the intended commit SHA is known.

Cargo documentation for publication:

https://doc.rust-lang.org/cargo/commands/cargo-publish.html

crates.io publishing guidance:

https://doc.rust-lang.org/cargo/reference/publishing.html

## Recommended development rhythm

During active coding:

```bash
cargo fmt --all
./scripts/<crate-name>-gates quick
```

Before a checkpoint:

```bash
./scripts/<crate-name>-gates full
./scripts/release-gates quick
```

Then inspect the diff, commit, push, and require green GitHub CI.

Before publication:

```bash
./scripts/<crate-name>-gates release
```

## Failure-handling philosophy

A gate failure is useful information, not something to bypass.

When a gate fails:

1. identify the first failing gate;
2. fix the underlying problem;
3. rerun the narrowest relevant command;
4. rerun the crate's full gate;
5. rerun wider workspace validation when shared boundaries changed;
6. push only after local validation is green;
7. require a green GitHub Actions run on the updated commit.

Avoid weakening strict gates just to obtain a green result.

Examples of useful failures already caught by this approach include:

- invalid `const fn` use against non-const dependency methods;
- an oversized `Result` error variant caught by strict Clippy;
- an ambiguous rustdoc link visible during workspace-wide strict documentation;
- MSRV-sensitive dependency resolution issues.

## Contributor checklist

Use this checklist when creating a new companion crate.

- [ ] Create a dedicated branch from current `main`.
- [ ] Add the crate under `crates/`.
- [ ] Register it as a workspace member.
- [ ] Add complete crates.io package metadata.
- [ ] Keep framework/runtime dependencies out of core.
- [ ] Add README documentation.
- [ ] Add crate-level Rust documentation.
- [ ] Document privacy, safety, trust, and failure boundaries.
- [ ] Add meaningful public-behavior tests.
- [ ] Create `scripts/<crate-name>-gates` immediately.
- [ ] Implement `quick`, `full`, and `release` modes.
- [ ] Enforce `cargo fmt --check`.
- [ ] Enforce strict Clippy with `-D warnings`.
- [ ] Run crate tests.
- [ ] Run doctests.
- [ ] Enforce crate-local strict rustdoc.
- [ ] Enforce workspace-wide strict rustdoc with `--all-features`.
- [ ] Verify Rust 1.85 MSRV compilation.
- [ ] Verify tests on Rust 1.85.
- [ ] Verify rustdoc on Rust 1.85.
- [ ] Inspect package contents.
- [ ] Register the crate in workspace release gates.
- [ ] Run the workspace quick gate after shared changes.
- [ ] Inspect the final diff boundary.
- [ ] Commit a coherent checkpoint.
- [ ] Push the branch.
- [ ] Require green GitHub CI on the exact checkpoint.
- [ ] Run the crate release gate before publication.
- [ ] Keep real publication outside automation.

## External reference index

### Rust and Cargo

- Cargo Book: https://doc.rust-lang.org/cargo/
- Cargo command reference: https://doc.rust-lang.org/cargo/commands/index.html
- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo manifests: https://doc.rust-lang.org/cargo/reference/manifest.html
- Cargo features: https://doc.rust-lang.org/cargo/reference/features.html
- Cargo SemVer compatibility: https://doc.rust-lang.org/cargo/reference/semver.html
- `cargo check`: https://doc.rust-lang.org/cargo/commands/cargo-check.html
- `cargo test`: https://doc.rust-lang.org/cargo/commands/cargo-test.html
- `cargo doc`: https://doc.rust-lang.org/cargo/commands/cargo-doc.html
- `cargo package`: https://doc.rust-lang.org/cargo/commands/cargo-package.html
- `cargo publish`: https://doc.rust-lang.org/cargo/commands/cargo-publish.html
- Publishing crates: https://doc.rust-lang.org/cargo/reference/publishing.html

### Formatting, linting, and documentation

- rustfmt: https://github.com/rust-lang/rustfmt
- Clippy: https://doc.rust-lang.org/clippy/
- rustdoc: https://doc.rust-lang.org/rustdoc/
- rustdoc lints: https://doc.rust-lang.org/rustdoc/lints.html
- Documentation tests: https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html
- Intra-doc links: https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html

### Toolchains and MSRV

- rustup book: https://rust-lang.github.io/rustup/
- rustup toolchains: https://rust-lang.github.io/rustup/concepts/toolchains.html

### Git and GitHub

- Git documentation: https://git-scm.com/doc
- GitHub Actions workflows: https://docs.github.com/en/actions/concepts/workflows-and-actions/workflows
- GitHub Actions syntax: https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
- GitHub Actions reference: https://docs.github.com/en/actions/reference
- GitHub CLI: https://cli.github.com/manual/
- GitHub CLI workflow runs: https://cli.github.com/manual/gh_run

## Related diagprint documentation

- [`COMPANION_CRATE_STANDARD.md`](COMPANION_CRATE_STANDARD.md) — mandatory project policy for companion crates.
- [`../scripts/README.md`](../scripts/README.md) — repository-local gate runner usage.

The intent is simple: a contributor should be able to create or modify a companion crate, run one documented local workflow, understand every command involved, and arrive at the same quality boundary enforced by the diagprint project and GitHub CI.
