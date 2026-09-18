# diagprint-snafu

Structured SNAFU interoperability for diagprint.

The adapter keeps custom `#[derive(Snafu)]` errors typed while attaching
diagprint reports and M4 source-chain relationships.

Default synchronous surface:

- `SnafuDiagnostic` and `SnafuDiagnosticMetadata`;
- `CapturedSnafuError<E>`;
- `DiagprintResultExt`;
- `DiagprintOptionExt`.

Opt-in `futures` surface:

- `DiagprintTryFutureExt`;
- `DiagprintTryStreamExt`;
- lazy direct capture;
- lazy SNAFU context construction;
- independent capture of each stream error;
- no Tokio dependency and no mandatory future/stream boxing.

SNAFU's implicit location data in Future/Stream context combinators follows
poll/combinator execution rather than necessarily identifying the call site
where the combinator was created. diagprint identity does not depend on this
location.

Advanced Whatever/privacy/backtrace policy remains deferred to E2C.
