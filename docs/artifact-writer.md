# Transactional artifact persistence

diagprint separates event sinks from complete durable artifacts.

> A sink receives events. An exporter produces an artifact.

`ArtifactWriter` persists an `ExportedArtifact` and its
`diagprint.receipt/v1` receipt as one create-only artifact directory.

## Layout

Example:

```text
artifacts/pr-42/
├── delta.json
└── delta.json.receipt.json
```

The directory is the transaction boundary.

A destination directory is considered committed only after the complete staged
directory has been renamed into its final location.

## Write protocol

`ArtifactWriter` performs the following steps:

1. Verify the in-memory artifact against its `ExportReceipt`.
2. Validate the final destination and artifact filename.
3. Create the destination parent directory if needed.
4. Create a hidden sibling staging directory.
5. Write the exact artifact bytes into the staging directory.
6. Flush and synchronize the artifact file.
7. Read the staged artifact back from storage.
8. Verify the staged bytes against the receipt.
9. Serialize the receipt as JSON.
10. Write, flush, and synchronize the receipt file.
11. Synchronize the staging directory where supported.
12. Confirm that the final destination has not appeared concurrently.
13. Rename the complete staging directory to the final destination.
14. Synchronize the destination parent directory where supported.

The final rename is the commit point.

## Why the directory is the transaction boundary

An artifact and its receipt are logically one durable unit.

Writing them directly to two final paths could expose an intermediate state
such as:

```text
delta.json
```

without:

```text
delta.json.receipt.json
```

or the inverse.

Instead, diagprint stages both files beneath a temporary sibling directory:

```text
artifacts/
└── .diagprint-stage-0199.../
    ├── delta.json
    └── delta.json.receipt.json
```

Only after both files have been written, synchronized, and verified is that
directory renamed to:

```text
artifacts/
└── pr-42/
    ├── delta.json
    └── delta.json.receipt.json
```

Consumers therefore do not intentionally observe a partially committed
artifact pair at the requested destination.

## Create-only semantics

`ArtifactWriter` does not intentionally overwrite an existing destination.

If:

```text
artifacts/pr-42/
```

already exists, another write to that destination fails with
`ArtifactWriteError::DestinationExists`.

This is deliberate.

Safe replacement of an existing artifact set requires generation, manifest, or
current-pointer semantics. diagprint models those separately rather than
pretending independent file replacement is transactional.

## Concurrent writers

The writer checks for an existing destination before staging and immediately
before the commit operation.

The final filesystem rename remains the authoritative commit operation.

If another process wins the race and creates the destination first, the losing
writer must not intentionally replace the completed destination.

Its uncommitted staging directory is cleaned up during ordinary error handling.

## Same-filesystem commit

The staging directory is created as a sibling of the destination:

```text
artifacts/
├── .diagprint-stage-0199...
└── pr-42/
```

This avoids intentionally performing the final rename across filesystem
boundaries.

The directory transaction therefore relies on ordinary same-filesystem rename
semantics.

## Staging cleanup

Before commit, the staging directory is owned by an internal cleanup guard.

On ordinary failures, dropping that guard removes the incomplete staging
directory recursively.

After a successful final rename, the guard is marked committed and no cleanup
is attempted.

A process or system crash can still leave a hidden staging directory:

```text
.diagprint-stage-...
```

Such a directory is not considered committed because it never appeared under
the requested final destination name.

Future generation or manifest tooling may add explicit stale-stage recovery.

## File synchronization

Each staged file is:

1. created with create-new semantics;
2. completely written;
3. flushed;
4. synchronized with `sync_all`.

The staged artifact is then read back and verified against its receipt before
the transaction is committed.

Where supported, diagprint also synchronizes:

- the staging directory before the final rename;
- the destination parent directory after the final rename.

This strengthens durability of both file contents and filesystem directory
entries.

## Integrity verification

`ArtifactWriter` verifies integrity twice.

First, before persistence:

```text
ExportedArtifact
    ↓
ExportReceipt::verify_bytes
```

This prevents an already inconsistent artifact/receipt pair from being written.

Second, after staging:

```text
write artifact
    ↓
sync
    ↓
read artifact back
    ↓
ExportReceipt::verify_bytes
```

This verifies the exact bytes staged on disk before commit.

Receipt verification checks:

- exact artifact byte length;
- exact SHA-256 artifact digest.

## Identity layers

Transactional persistence deliberately preserves diagprint's separate identity
layers.

### DiagnosticFingerprint

Answers:

> What logical diagnostic is this?

Defined by:

```text
diagprint.canonical/v1
```

### DiagnosticDigest

Answers:

> What exact meaningful diagnostic content is this?

Defined by:

```text
diagprint.canonical/v1
```

### ReportDigest

Answers:

> What meaningful diagnostic report state is this?

Defined by:

```text
diagprint.canonical/v1
```

### ArtifactDigest

Answers:

> What exact external bytes were produced?

Recorded by:

```text
diagprint.receipt/v1
```

A compact JSON artifact and a pretty JSON artifact may therefore represent the
same semantic report while having different artifact digests.

## Receipt persistence

The receipt filename is derived from the artifact filename.

For:

```text
delta.json
```

the receipt is:

```text
delta.json.receipt.json
```

The persisted receipt records:

- receipt schema;
- artifact schema;
- media type;
- encoding;
- exact SHA-256 artifact digest;
- exact byte length;
- canonical baseline report digest;
- canonical candidate report digest;
- privacy-safe export-policy description;
- optional CI evaluation result.

## Artifact filename safety

`artifact_name` must be exactly one normal filesystem path component.

Valid examples:

```text
delta.json
report.json
analysis.sarif
diagnostics.md
```

Rejected examples:

```text
../delta.json
subdir/delta.json
./delta.json
```

This prevents the artifact filename from escaping the transaction directory.

The transaction destination itself remains a caller-selected path.

## Privacy boundary

`ArtifactWriter` does not reinterpret or expand exported diagnostic data.

The privacy boundary has already been applied before persistence:

```text
Diagnostic
    ↓
ExportPolicy
    ↓
ExportDiagnostic
    ↓
diagprint.delta/v1
    ↓
exact artifact bytes
    ↓
diagprint.receipt/v1
    ↓
ArtifactWriter
```

The writer persists those exact bytes.

It does not regain access to omitted source text, source-cache contents,
sensitive process metadata, remediation edit payloads, or suggested command
contents.

## Reporter separation

`Reporter` remains an interactive and event-oriented facility.

Its responsibilities include live terminal rendering and event-style
diagnostic output.

Complete durable artifacts are not append events.

Transactional artifacts must therefore not be appended into Reporter log
files.

Without this separation, a single file could accidentally become:

```text
plain diagnostic
plain diagnostic
JSON document
SARIF document
Markdown document
```

which is not a valid artifact in any of those formats.

The architectural rule is:

> A sink receives events. An exporter produces an artifact.

`ArtifactWriter` belongs to the artifact side of that boundary.

## Future generations and manifests

Create-only transactions are the foundation for versioned artifact history.

A future layout can safely use immutable generations:

```text
artifacts/
├── generations/
│   ├── 000001/
│   │   ├── delta.json
│   │   └── delta.json.receipt.json
│   ├── 000002/
│   │   ├── delta.json
│   │   └── delta.json.receipt.json
│   └── 000003/
│       ├── delta.json
│       └── delta.json.receipt.json
└── manifest.json
```

The manifest can identify the current immutable generation without modifying
an already committed artifact transaction.

That enables:

- immutable history;
- rollback;
- retention;
- reproducibility;
- safe current-generation selection;
- later `.diagpack` bundle construction.

Replacement and versioning therefore build on top of `ArtifactWriter` instead
of weakening its create-only guarantees.
