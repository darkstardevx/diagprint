# diagprint forensic timeline v1

Schema:

```text
diagprint.forensics.timeline/v1
```

Timeline v1 is a deterministic run-by-run representation of one logical
diagnostic fingerprint across a verified diagprint diagnostic history.

It complements `diagprint.forensics.case-file/v1`:

- the case file summarizes the lifetime of a finding;
- the timeline retains one state row for every history run.

## Evidence boundary

Timeline v1 derives only from verified `DiagnosticHistory` data.

It inherits the same privacy-light boundary as history and case-file v1.

It does not regain:

- diagnostic messages;
- source paths;
- source text;
- arbitrary diagnostic attributes;
- help;
- notes;
- cause chains;
- remediation payloads;
- suggested commands.

## Phases

Each retained run has exactly one phase.

### `unseen`

The fingerprint has not yet appeared in any retained run.

An unseen run is not called resolved because there is not yet an observed
diagnostic to resolve.

### `active`

One or more instances matching the fingerprint are present.

### `absent`

The fingerprint has previously appeared but is not present in this run.

This includes the resolution transition and subsequent clean runs.

## Events

Timeline events are deterministic observations.

### `first_seen`

The first retained active run for the fingerprint.

### `persisting`

The diagnostic remained active from the previous run without a more
significant transition.

### `changed`

At least one matching logical diagnostic changed canonical content while
retaining fingerprint identity.

### `severity_increased`

At least one matching logical diagnostic increased severity.

### `resolved`

The immediately preceding run was active and the current run is absent.

### `reappeared`

The diagnostic became active after one or more absent runs.

A reappearance begins a new episode.

## Episodes

Active runs are grouped into one-based contiguous episodes.

Every active run carries its episode number.

Unseen and absent runs carry no episode number.

Episode semantics are shared with case-file v1.

## Clean windows

A clean window is a contiguous interval where the fingerprint is not active.

### `before_first_seen`

Retained history before the first observation.

### `between_episodes`

A clean interval separating two active episodes.

This is especially useful for identifying regressions after a period where the
finding was absent.

### `after_resolution`

Trailing clean history after the final retained active episode.

## Transition counts

Every timeline run retains existing lineage transition counts:

- newly introduced instances;
- resolved instances;
- persisting instances;
- changed instances;
- severity increases.

Timeline v1 does not redefine canonical identity or semantic delta behavior.

## CLI glyphs

`diagprint timeline` uses a compact presentation layer:

```text
●  first observation or ordinary active run
▲  severity increase
◆  canonical content change
○  resolution transition
↻  reappearance after absence
·  absent or not-yet-seen run
```

Glyph precedence affects presentation only.

Every underlying event and transition count remains available in the timeline
model.

## Verification

The CLI re-verifies the persisted diagnostic-history chain immediately before
constructing the timeline.

`chain-verified: true` means that verification succeeded for the local retained
history at presentation time.

The history chain remains local integrity evidence rather than authenticated
remote attestation.

## No source-control inference

Timeline v1 makes no claims about:

- introducing commit;
- responsible author;
- Git blame;
- root cause;
- probable fix;
- remediation success.

Source-control provenance belongs to a later forensic schema.

## Compatibility

The meaning of `diagprint.forensics.timeline/v1` is immutable.

Backward-incompatible semantic changes require a new schema version.
