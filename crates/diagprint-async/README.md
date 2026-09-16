# diagprint-async

Bounded asynchronous diagnostic delivery for the `diagprint` diagnostics
lifecycle framework.

The core `diagprint` crate remains synchronous and runtime-independent.
`diagprint-async` provides an explicit Tokio-based delivery layer when
applications need asynchronous submission.

## Highlights

- strictly bounded queues
- explicit backpressure policy
- ordered diagnostic delivery
- asynchronous flush
- orderly shutdown
- worker failure propagation
- diagnostic report submission
- no unbounded buffering

Available overload policies are:

- `Block`
- `Reject`
- `DropNewest`

`DropNewest` only drops diagnostics at or below its configured severity
threshold. Higher-severity diagnostics are rejected rather than silently lost.

## Installation

    [dependencies]
    diagprint = "0.7"
    diagprint-async = "0.7"

## Main API

The primary types are:

- `AsyncDiagnosticSink`
- `BackpressurePolicy`
- `SubmitOutcome`
- `SubmissionSummary`
- `AsyncSinkError`
- `ReportSubmitError`

An active Tokio runtime is required when spawning the asynchronous sink.

## MSRV

Rust 1.85 or newer.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
