# `diagprint.canonical/v1`

`diagprint.canonical/v1` is the first stable canonical identity schema for
`diagprint` diagnostics and reports.

Its purpose is not presentation. It exists so later lifecycle features can
reliably answer questions such as:

- Is this the same logical diagnostic as one seen in an earlier run?
- Did the meaningful content of a diagnostic change?
- Are two reports content-equivalent even if diagnostics were inserted in a
  different order?
- Which exact report produced an exported artifact?

Once released, the meaning and byte encoding of canonical v1 are immutable.
Any incompatible semantic or encoding change requires a new canonicalization
version such as `diagprint.canonical/v2`.

## Hash algorithm

Canonical v1 uses SHA-256.

Digest values expose three pieces of identity independently:

- canonicalization: `diagprint.canonical/v1`
- algorithm: `sha256`
- 32-byte digest value

The human-readable short form is `sha256:<64 lowercase hex digits>`.

## Domain separation

Every canonical hash starts with two length-prefixed UTF-8 tokens:

1. `diagprint.canonical/v1`
2. one domain string

Canonical v1 defines these domains:

- `diagprint.diagnostic-fingerprint/v1`
- `diagprint.diagnostic-digest/v1`
- `diagprint.report-digest/v1`

This prevents bytes valid for one identity purpose from being interpreted as
another identity purpose.

## Primitive encoding

Canonical v1 is a binary encoding written directly into the SHA-256 state.
It does not use JSON, Serde field ordering, `Debug`, or Rust enum discriminants.

- Field names and UTF-8 strings are prefixed by a 128-bit unsigned big-endian
  byte length.
- Sequence counts are encoded as 128-bit unsigned big-endian integers.
- `u32`, `u64`, `u128`, `i64`, and `i128` use fixed-width big-endian bytes.
- `usize` values are promoted losslessly to `u128` before encoding.
- `bool` is one byte: `0` or `1`.
- `Option<T>` begins with one byte: `0` for `None`, `1` for `Some`, followed by
  the value when present.
- `f64` is encoded by IEEE-754 bits in big-endian order, with all NaN payloads
  normalized to `0x7ff8000000000000` and both positive and negative zero
  normalized to zero.
- Remediation `Path` values must be valid UTF-8. Canonicalization fails closed
  rather than using a lossy conversion.

## Diagnostic fingerprint

`DiagnosticFingerprint` answers:

> What logical diagnostic is this?

It is intentionally less sensitive than `DiagnosticDigest` so a diagnostic can
remain identifiable after its severity, exact location, help text, or proposed
fix changes.

### Default derived fingerprint

When no explicit identity is present, the v1 default fingerprint includes:

- diagnostic code, if present;
- diagnostic message;
- every primary label's final filename component;
- every primary label message.

Primary-label identity tuples are sorted before hashing. Line, column, label
length, and source revision are excluded.

The default filename projection treats `/` and `\\` as separators so common
Unix and Windows-style source names have portable basename behavior.

`FingerprintPolicy` can independently omit the code, message, source identity,
or primary-label messages. Source identity can be omitted, reduced to the final
filename, or use the full stored source name.

### Explicit producer identity

The reserved structured attribute name is:

`diagprint.identity`

When one or more such attributes are present and explicit identity is enabled,
canonical v1 enters explicit-identity mode. The fingerprint contains the
optional diagnostic code plus the sorted typed identity attribute values.
Message and source location are not included in that mode.

This lets an integration that already owns a stable rule/occurrence identity
keep the fingerprint stable across wording and location changes without adding
new mutable fields to `Diagnostic`.

## Diagnostic digest

`DiagnosticDigest` answers:

> What meaningful diagnostic content is this?

Canonical v1 includes:

- application;
- severity;
- code;
- message;
- all structured attributes;
- labels, including kind, exact stored source name, line, column, length, and
  label message;
- notes in their stored order;
- help;
- the ordered cause chain;
- suggestions in their stored order;
- suggestion applicability;
- documentation labels, URLs, and language hints;
- complete structured edit payloads;
- suggested command contents and explanations.

Structured attributes are sorted by attribute name, value type, and canonical
value bytes before hashing. This makes attribute insertion order irrelevant
while preserving duplicate attributes.

Other ordered collections remain ordered because their order can carry
presentation or remediation meaning.

### Deliberately excluded fields

Canonical v1 diagnostic digests exclude occurrence/provenance fields:

- `report_id`;
- `session_id`;
- timestamp;
- PID;
- hostname;
- source revision numbers.

Those values describe when and where an observation happened, not the semantic
content of the diagnostic. Excluding them allows identical diagnostic content
to have the same digest across runs and machines when the stored semantic source
identifiers are the same.

Source revisions remain available to the live diagnostic and future bundle
provenance manifests; they are simply not part of content identity.

## Source text

Canonical v1 never reads source contents from `SourceCache`, `SourceSnapshot`,
or the filesystem.

A digest may include source fragments that already exist inside a structured
remediation edit, such as an edit's expected/replacement text. It does not pull
additional source text into identity calculation.

Future bundle/source-verification work should use separate source-content
digests rather than changing canonical v1 diagnostic semantics.

## Report digest

`ReportDigest` answers:

> What diagnostic content does this report contain?

For every diagnostic in the report:

1. compute its canonical v1 `DiagnosticDigest`;
2. sort the 32-byte digest values lexicographically;
3. preserve duplicates;
4. encode the diagnostic count and every sorted digest;
5. hash them under `diagprint.report-digest/v1`.

Therefore diagnostic insertion order does not affect `ReportDigest`, while any
meaningful diagnostic-content change does.

## Compatibility rule

Implementations must never silently alter canonical v1 behavior.

Changes such as adding a field to a digest, changing ordering rules, changing
float normalization, or changing path handling require a new canonicalization
version. Regression vectors in the test suite protect the v1 byte contract.

## Serialization independence

`diagprint.canonical/v1` does not derive canonical bytes from Serde or another
general-purpose serialization representation.

Canonical identity input MUST NOT be produced from:

- Serde serialization;
- JSON or another interchange format;
- `Debug` formatting;
- `Display` formatting;
- Rust enum discriminant values;
- Rust struct memory layout.

Serde implementations on `DiagnosticFingerprint`, `DiagnosticDigest`, and
`ReportDigest` serialize completed identity values for interchange only. They
do not define or influence canonical-v1 bytes.

Deterministic sort keys used internally to order unordered collections are
also distinct from canonical field encoding.
