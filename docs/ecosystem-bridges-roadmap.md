# Ecosystem Bridges

## Product thesis

> Use the error and diagnostic tools you already like. Diagprint connects them
> into one structured diagnostic lifecycle.

diagprint is not trying to replace every Rust error, diagnostics, tracing,
compiler, editor, or telemetry crate.

The interoperability goal is to preserve the useful structured information
those systems already know and connect it to diagprint's lifecycle:

```text
normalize
  │
understand
  │
act
  │
export
```

## Reusable bridge SDK

`diagprint-bridge` is the reusable interoperability construction SDK shared by
Ecosystem Bridge adapters.

It builds on core `InteropDiagnostic` plus the M4 relationship graph rather
than replacing either one.

The SDK owns common adapter mechanics such as normalized mapper output,
ephemeral construction node handles, report/graph assembly, logical
self-relation collapse, bridge statistics, and shared bridge errors.

Adapter-specific crates continue to own ecosystem-specific traversal, typed
source views, and semantics.

`BridgeNodeId` is intentionally ephemeral. Durable graph identity remains the
canonical diagnostic fingerprint.

## Bridge rule

A good bridge does not reduce an upstream error to:

```text
other_error.to_string()
```

when structured information is available.

Adapters should preserve, when available:

- stable identity;
- diagnostic/error code;
- severity;
- source locations;
- typed context;
- source/error chains;
- attachments;
- relationship structure;
- trace/span context;
- remediation metadata;
- producer provenance.

Unsupported information should remain unsupported rather than guessed.

## Architecture policy

Small, stable, low-dependency adapters may be optional core features.

Large, fast-moving, domain-specific, or dependency-heavy adapters should be
companion crates.

The core interoperability model must not depend on any one third-party error
ecosystem.

## Interleaved roadmap

```text
M4    Causal Diagnostic Graph                   COMPLETE
E1A0  diagprint-bridge interoperability SDK     COMPLETE
E1    error-stack                               COMPLETE
M5    Replay / regression / remediation evidence M5B / FINAL CI CHECKPOINT
E2    SNAFU                                     uses bridge SDK
E3    eyre / color-eyre                         uses bridge SDK
E4    tracing-error                             uses bridge SDK
```

Additional bridges are selected based on ecosystem value, maintenance quality,
public structured APIs, MSRV fit, and dependency cost.

## Milestone M5 — remediation evidence replay

M5 has reached its final implementation CI checkpoint.

M5A delivered append-only history-bound remediation evidence. M5B adds
read-only per-fingerprint replay, later-reappearance analysis, strict
verified-regression semantics, deterministic text/JSON CLI output, and
remediation sidecar verification.

It connects the existing `FixPlan`, `RemediationReceipt`, semantic delta, and
tamper-evident history layers without creating another remediation engine.

The central rule is:

```text
replay evidence, not edits
```

A remediation evidence record must bind an exact receipt to two adjacent
verified history runs and require the receipt's aggregate semantic effect to
match the history transition exactly.

`diagprint replay` will be read-only forensic reconstruction. It may report
that the same canonical diagnostic reappeared after a verified remediation and
observed resolution, but it will not claim that remediation caused the original
resolution or that the recurrence has the same root cause.

## Bridge E1 — error-stack

E1 is complete.

E1A0 delivered the ecosystem-neutral `diagprint-bridge` SDK, and E1 delivered
`diagprint-error-stack` as its first concrete consumer.

The completed adapter supports single and grouped reports, preserves stable
structured source topology through the M4 relationship graph, and keeps
attachment contents private by default.

The implementation uses stable public `error-stack` APIs and does not depend on
nightly-only attachment-provider APIs.

M5 replay / regression / remediation evidence is the next interleaved feature
train. E2 SNAFU remains the next planned adapter and will reuse the bridge SDK.

Goals:

- preserve structured report/frame context where public APIs permit;
- preserve source-chain relationships;
- map stable structure into M4 relationship evidence;
- avoid flattening attachments unnecessarily;
- keep adapter dependency isolated from users who do not enable it.

Preferred packaging will be decided by the E1 plan after evaluating the exact
dependency and feature surface at implementation time.

## Bridge E2 — SNAFU

Goals:

- preserve typed context and source relationships;
- map useful stable context into diagprint diagnostics;
- feed structured relationship evidence into the M4 graph;
- avoid relying on rendered error strings when typed/source APIs suffice.

## Bridge E3 — eyre / color-eyre

Goals:

- provide application-facing report interoperability;
- consume available error-chain/context information without leaking eyre
  concrete report types into unrelated diagprint APIs;
- preserve diagprint's structured model even when upstream information is
  necessarily dynamic.

## Bridge E4 — tracing-error

Goals:

- connect span-context evidence to diagnostics;
- classify trace relationships distinctly from causal relationships;
- never equate span ancestry or temporal proximity with root cause.

Because tracing-error describes itself as experimental, integration should be
conservative and isolated.

## Evidence vocabulary

Bridges feed the same graph model rather than inventing private relationship
formats.

Examples:

```text
producer_declared
source_chain
structural
trace_context
temporal_association
inferred_correlation
```

Evidence provenance and relation semantics remain separate.

## Long-term outcome

The same logical diagnostic should be able to move between systems without
discarding its lifecycle:

```text
third-party crate
      │
      ▼
diagprint interoperability
      │
      ├── canonical identity
      ├── history
      ├── why
      ├── timeline
      ├── graph
      ├── Git provenance
      ├── replay / remediation
      └── export
             │
             ├── terminal
             ├── JSON
             ├── SARIF
             ├── LSP
             ├── OpenTelemetry
             └── capsules
```
