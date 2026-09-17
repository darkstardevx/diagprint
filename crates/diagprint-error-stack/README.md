# diagprint-error-stack

Structured [`error-stack`](https://crates.io/crates/error-stack)
interoperability for [`diagprint`](https://crates.io/crates/diagprint).

This is the first ecosystem adapter built on the reusable
[`diagprint-bridge`](https://crates.io/crates/diagprint-bridge) SDK.

The adapter keeps `error-stack`-specific traversal and privacy policy local
while the SDK owns normalized metadata, diagnostic report assembly, logical
identity handling, and M4 relationship graph construction.

## Supported reports

Both stable report forms are supported:

```text
Report<C>    single current context
Report<[C]> grouped current contexts
```

Grouped reports are traversed from every `current_frames()` root. Independent
siblings are not related merely because they occur in one grouped report.

## Structure, not rendering

The adapter uses stable structured APIs:

- `Report::current_frame()`;
- `Report::current_frames()`;
- `Frame::kind()`;
- `Frame::sources()`;
- `Frame::downcast_ref::<T>()`.

It does **not** parse terminal rendering, ANSI output, box-drawing characters,
alternate `Display`, or `Debug` output.

## Relationship semantics

Structured source contexts map as:

```text
deeper/source context
    -- contributes_to / source_chain / error-stack -->
outer/current context
```

The adapter does not invent a stronger `causes` edge by default.

## Attachment privacy

Attachment content is omitted by default.

To intentionally include printable attachment text:

```rust
use diagprint_error_stack::{
    ErrorStackAttachmentPolicy,
    ErrorStackReportExt,
};

let output = report.to_diagprint_with_policy(
    &reporter,
    ErrorStackAttachmentPolicy::PrintableText,
)?;
```

Only `AttachmentKind::Printable` text is included, as diagnostic notes on the
nearest context below the attachment in that source branch.

Opaque attachment values are never exported by this adapter.

Attachment frames remain traversal-transparent, so source relationships are
preserved regardless of whether attachment content is included.

## SDK boundary

`diagprint-error-stack` delegates common mechanics to `diagprint-bridge`:

- normalized mapper output;
- ephemeral construction node handles;
- canonical logical identity attachment;
- `DiagnosticReport` construction;
- M4 relationship validation and graph construction;
- duplicate logical-instance handling;
- logical self-relation collapse;
- generic bridge statistics.

## Backtrace behavior

`diagprint-error-stack` depends on `error-stack` with default features disabled
and only `std` enabled.

Backtraces and span traces are not automatically exported.

## MSRV

`diagprint-error-stack` follows the diagprint workspace MSRV: Rust 1.85.
