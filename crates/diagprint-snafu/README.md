# diagprint-snafu

Structured SNAFU interoperability for diagprint.

E2A provides the synchronous foundation:

- typed `#[derive(Snafu)]` application errors through `SnafuDiagnostic`;
- stable `SnafuIdentity` and `SnafuCode` classification;
- `CapturedSnafuError<E>` preserving the original concrete error;
- `Result` and `Option` extension traits;
- source-chain conversion into diagprint's M4 relationship graph.

Future/Stream extensions and advanced Whatever/privacy/backtrace policy are intentionally deferred to later E2 checkpoints.
