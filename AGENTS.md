# diagprint Agent Architecture and Development Contract

Read this before modifying Rust code.

## Core rules

- MSRV is Rust 1.85.
- Keep diagnostic construction, canonical identity, rendering, source capture,
  remediation, history, forensics, Git provenance, and capsules separate.
- Do not silently redefine versioned schemas.
- History labels are opaque caller-controlled strings.
- Provenance is not causation.
- Privacy-light forensic storage must not regain diagnostic/source payloads
  unless a new explicit schema permits it.
- Rust implementation requires an Approved plan already committed in HEAD.
- Coding agents must not bypass hooks with `git commit --no-verify`.
- After a failed mutating script, inspect current state and repair forward.
- Do not blindly reset or clean the repository.

## Main data flow

Diagnostic / Reporter -> DiagnosticReport -> canonical identity / render / history

DiagnosticHistory -> why -> timeline -> Git provenance / blame

Remediation is separate:
Diagnostic -> Suggestion -> Fixer/FixPlan -> RemediationReceipt

Source capture is separate:
SourceCache -> SourceSnapshot -> SourceRevision -> CapturedDiagnostic

## Important modules

- src/diagnostic.rs
- src/reporter.rs
- src/report.rs
- src/canonical.rs
- src/fingerprint.rs
- src/history.rs
- src/forensics.rs
- src/git_provenance.rs
- src/capsule.rs
- src/source.rs
- src/captured.rs
- src/fixer.rs
- src/fixplan.rs
- src/project_scan/
- src/render/
- src/bin/diagprint.rs

## Immutable schema boundaries

- diagprint.canonical/v1
- diagprint.history.run/v2
- diagprint.forensics.case-file/v1
- diagprint.forensics.timeline/v1
- diagprint.forensics.git-provenance/v1
- diagprint.capsule/v1

Backward-incompatible changes require a new schema version.

## Forensics roadmap

Completed:
- M1: diagprint why
- M2: diagprint timeline
- M3: diagprint blame

Next:
- M4: causal diagnostic graph
- M5: replay / regression / remediation evidence

## Plan-first workflow

The active plan pointer is `.plans/ACTIVE`.

The referenced plan must already exist in HEAD with:

Status: Approved

The pre-commit hook enforces that for Rust changes.

Typical flow:

1. `./scripts/plan new M4-causal-graph`
2. Edit the plan.
3. `./scripts/plan approve`
4. Commit `.plans/`.
5. Implement Rust.
6. Commit implementation.
7. Run full gates and CI.

## Development gates

- `./scripts/gate.sh precommit`
- `./scripts/gate.sh fast`
- `./scripts/gate.sh forensics`
- `./scripts/gate.sh full`

CI remains authoritative.

## Generated text

Text files must end with exactly one real newline.

Correct Python normalization:

`text.rstrip("\n") + "\n"`

Do not use escaped literal backslash-n strings.

Prefer structural patch checks over formatting-sensitive anchors.
