# `diagprint.delta/v1`

`diagprint.delta/v1` is the portable semantic report-delta artifact schema.

It records the result of comparing two diagnostic reports using
`diagprint.canonical/v1` identity.

## Identity

Every artifact records:

- the canonical baseline `ReportDigest`;
- the canonical candidate `ReportDigest`;
- the exact fingerprint policy used for logical matching;
- each diagnostic fingerprint;
- baseline and candidate diagnostic content digests where applicable.

The artifact does not redefine canonical identity.

`diagprint.canonical/v1` remains the authority for fingerprint and digest
semantics.

## Delta classifications

Entries are classified as:

- `new`;
- `resolved`;
- `persisting`;
- `changed`.

Duplicate diagnostics are preserved using multiset semantics.

Exact digest matches are paired before remaining same-fingerprint diagnostics
are classified as changed.

## Privacy boundary

Diagnostic payloads MUST cross the delta artifact boundary through
`ExportDiagnostic` and an explicit `ExportPolicy`.

The delta artifact MUST NOT directly serialize internal `Diagnostic` values.

The default export policy therefore retains the existing diagprint external
export guarantees, including:

- filename-only source paths;
- omitted arbitrary attribute values;
- metadata-only remediation;
- sanitized documentation URLs;
- omitted hostname and PID;
- no source-cache contents;
- no remediation edit payloads;
- no suggested command contents.

Canonical fingerprints and digests remain available because they identify
diagnostic state without exporting the underlying sensitive content used to
compute them.

## CI evaluation

An artifact may contain a `DeltaPolicy` evaluation.

When present it records:

- the exact configured policy thresholds;
- success or failure;
- the conventional process exit code;
- each violated rule;
- the number of matching diagnostic instances.

Policy evaluation does not alter delta classification.

## Determinism

For identical baseline and candidate diagnostic state, fingerprint policy,
delta policy, and export policy, serialization order is deterministic.

Report insertion order does not determine delta entry ordering.

JSON is an artifact encoding of this schema. JSON serialization does not define
canonical diagnostic identity.

## Compatibility

`diagprint.delta/v1` is versioned independently from
`diagprint.canonical/v1`.

Any incompatible structural or semantic change to the delta artifact contract
requires a new delta schema version rather than silently changing v1.
