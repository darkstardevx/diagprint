# Remediation Evidence and Replay v1

M5 connects successful guarded remediation to diagprint's tamper-evident
diagnostic history.

The governing principle is:

```text
replay evidence, not edits
```

M5 never re-executes an historical `FixPlan`.

## Remediation evidence schema

M5A defines:

```text
diagprint.remediation.evidence/v1
```

One evidence record binds:

- one exact successful remediation receipt;
- one exact pre-remediation history run;
- the immediately adjacent post-remediation history run;
- both history run digests;
- both canonical report digests;
- the exact remediation receipt digest;
- the exact fix-plan descriptor digest carried by that receipt;
- remediation status and verification summary;
- semantic before/after diagnostic effect.

Records are stored at:

```text
<HISTORY>/remediation/run-XXXXXX.json
```

where `XXXXXX` is the post-remediation run index.

## Exact-transition rule

Evidence is valid only for adjacent runs:

```text
before_run + 1 == after_run
```

The receipt's before/after report identities must exactly equal the report
identities in those two history runs.

The receipt's aggregate semantic effect must also exactly equal the transition
recomputed from history.

## Digest model

The exact compact JSON serialization of the typed `RemediationReceipt` is
hashed and stored as `receipt_digest`.

The evidence record itself has a separate `record_digest` covering every
evidence field except `record_digest`.

This lets history retain a privacy-light anchor to the exact receipt without
embedding the receipt itself.

## Persistence behavior

Evidence sidecars are append-only.

- first write uses create-new semantics;
- an identical retry is idempotent;
- a different record for the same post-remediation run is rejected;
- tampered record digests are rejected;
- history is re-verified before evidence is recorded or replayed.

## Privacy boundary

The v1 evidence sidecar does not persist:

- fix-plan title or explanation;
- source paths;
- changed-file paths;
- source text;
- edit replacement text;
- expected-before edit guards;
- verification payload text;
- diagnostic messages;
- help or notes;
- causes;
- arbitrary diagnostic attributes.

The record retains cryptographic identities, classifications, verification
summary, changed-file count, and aggregate diagnostic effects.

## M5B replay

M5B will derive:

```text
diagprint.forensics.remediation-replay/v1
```

from verified history plus verified remediation evidence.

Replay is read-only and will never invoke `FixPlan::apply`.
