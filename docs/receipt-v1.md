# `diagprint.receipt/v1`

`diagprint.receipt/v1` records the exact external artifact bytes produced by a
diagprint exporter.

A receipt is not a replacement for canonical diagnostic identity.

## Identity layers

diagprint deliberately separates three concepts.

### Diagnostic fingerprint

Answers:

> What logical diagnostic is this?

Defined by `diagprint.canonical/v1`.

### Report digest

Answers:

> What meaningful diagnostic report state is this?

Defined by `diagprint.canonical/v1`.

### Artifact digest

Answers:

> What exact bytes crossed the external export boundary?

The artifact digest is SHA-256 over the complete exported byte stream.

Compact JSON and pretty JSON therefore have different artifact digests even
when they represent the same diagnostic state.

## Receipt contents

A receipt records:

- receipt schema;
- artifact schema;
- media type;
- external encoding;
- exact artifact SHA-256 digest;
- exact artifact byte length;
- canonical baseline report digest;
- canonical candidate report digest;
- privacy-safe export-policy description;
- optional CI evaluation status and exit code.

## Privacy

Receipts MUST NOT contain source text, remediation edit payloads, command
contents, credentials, or absolute repository roots merely to identify an
export.

Repository-relative export records that repository-relative mode was used but
does not serialize the repository root path into the receipt.

The artifact digest still identifies the exact exported bytes.

## Verification

Verification MUST check both:

1. byte length;
2. artifact digest.

A matching report digest is not sufficient to prove that an external file is
the exact artifact originally exported.

## Compatibility

Incompatible receipt structure or semantics require a new receipt schema
version rather than silently changing `diagprint.receipt/v1`.
