# diagprint-error-stack

Structured [`error-stack`](https://crates.io/crates/error-stack)
interoperability for [`diagprint`](https://crates.io/crates/diagprint).

This is the first ecosystem adapter built on the reusable
[`diagprint-bridge`](https://crates.io/crates/diagprint-bridge) SDK.

The adapter keeps `error-stack`-specific traversal local while the SDK owns
normalized metadata, diagnostic report assembly, logical identity handling, and
M4 relationship graph construction.

## E1A support

E1A converts stable single-context `error_stack::Report<C>` values into:

- a `DiagnosticReport` with one diagnostic instance per context frame;
- a verified `DiagnosticRelationshipGraph`;
- reusable `BridgeBuildStats`;
- adapter-specific context/attachment frame counts.

Grouped `Report<[C]>` support and explicit attachment-content policy arrive in
E1B.

## Structure, not rendering

The adapter uses:

- `Report::current_frame()`;
- `Frame::kind()`;
- `Frame::sources()`;
- stable `Frame::downcast_ref::<T>()` for application mappers.

It does **not** parse `Report` terminal rendering, ANSI output, box-drawing
characters, or debug text.

## Relationship semantics

Structured source contexts map as:

```text
deeper/source context
    -- contributes_to / source_chain / error-stack -->
outer/current context
```

The adapter does not invent a stronger `causes` edge by default.

## Typed mapper

Implement `ErrorStackContextMapper` when application domain errors can provide
stronger metadata.

Mapper input is error-stack-specific:

```text
ErrorStackContextView
```

Mapper output is shared across adapters:

```text
diagprint_bridge::BridgeDiagnosticMetadata
```

A mapper can use `view.frame().downcast_ref::<T>()` to recognize known context
types and provide:

- severity;
- code;
- help;
- notes;
- stable application-owned logical identity.

## Attachments

E1A treats attachment frames as traversal-transparent.

Their content is not copied into diagnostics. The adapter follows their source
frames so context-to-context relationships remain intact.

E1B adds an explicit printable attachment opt-in policy and dedicated privacy
regression coverage.

## Backtrace behavior

`diagprint-error-stack` depends on `error-stack` with default features disabled
and only `std` enabled. Installing the adapter therefore does not intentionally
enable error-stack's default backtrace feature.

Backtraces and span traces are not automatically exported by this adapter.

## MSRV

`diagprint-error-stack` follows the diagprint workspace MSRV: Rust 1.85.
