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
M4  Causal Diagnostic Graph                     COMPLETE
E1  error-stack                                 ACTIVE PLAN
M5  Replay / regression / remediation evidence
E2  SNAFU
E3  eyre / color-eyre
E4  tracing-error
```

Additional bridges are selected based on ecosystem value, maintenance quality,
public structured APIs, MSRV fit, and dependency cost.

## Bridge E1 — error-stack

M4 is complete. E1 now has an active implementation plan.

The implementation plan will target the stable public structured surface of
`error-stack` first and will not depend on nightly-only attachment-provider APIs.

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
