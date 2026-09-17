# diagprint forensic case file v1

Schema:

```text
diagprint.forensics.case-file/v1
```

`diagprint.forensics.case-file/v1` is the first stable forensic interpretation
of a verified diagprint diagnostic-history chain.

Its purpose is to answer:

> What can the retained diagnostic evidence prove about this logical finding?

It deliberately does **not** answer:

> What probably caused this failure?

Causal and source-control analysis belong to later forensic layers and must
remain distinguishable from direct evidence.

## Evidence boundary

Case-file v1 consumes `DiagnosticHistory`.

It inherits the privacy-light boundary of diagnostic history.

It may contain:

- canonical diagnostic fingerprints;
- canonical diagnostic content digests;
- report digests;
- history run digests;
- history chain-head identity;
- run indexes;
- caller-supplied run labels;
- severity names and instance counts;
- deterministic lifecycle calculations.

It does not regain:

- diagnostic messages;
- source paths;
- source text;
- arbitrary attributes;
- help;
- notes;
- causes;
- remediation payloads;
- suggested commands.

## Identity

A case file describes exactly one full canonical diagnostic fingerprint.

CLI layers may resolve a unique shortened hexadecimal prefix before creating the
case file, but the serialized case file always contains the full canonical
fingerprint.

## Status

A case is `active` when at least one matching instance exists in the newest
history run.

A case is `resolved` when it was observed in at least one historical run but no
matching instance exists in the newest run.

A fingerprint never observed in history does not produce a case file.

## Evidence runs

`evidence` contains only runs where the fingerprint was present.

Each evidence run records:

- run index;
- opaque run label;
- report digest;
- run digest;
- matching instance count;
- per-severity instance counts;
- unique canonical diagnostic content digests.

Evidence remains ordered by history run index.

## Episodes

An episode is one contiguous interval of history runs where the fingerprint is
present.

An episode begins on the first active run following either:

- the beginning of history; or
- at least one absent run.

An episode ends when the first subsequent absent run is observed.

For a closed episode, `resolved_run` is that first absent run.

For an episode still active at the history head, `resolved_run` is `null`.

`reappearances` is:

```text
max(episode_count - 1, 0)
```

This gives reappearance a deterministic meaning rather than treating it as a
heuristic.

## Changes

`introduced_instances`, `resolved_instances`, `changed_instances`, and
`severity_increases` reuse the existing deterministic diagnostic-lineage
classification.

Case-file v1 does not redefine canonical identity or semantic delta behavior.

## Chain evidence

`chain_head` records the current diagnostic-history chain head.

The CLI re-verifies the on-disk history immediately before constructing a
case file and prints:

```text
chain-verified: true
```

when that verification succeeds.

The chain remains local integrity evidence rather than authenticated remote
attestation. An attacker able to rewrite all history state can recompute it;
stronger evidence requires an independently retained capsule or other anchor.

## Run labels

Run labels are opaque caller-controlled strings.

Case-file v1 does not interpret them as:

- timestamps;
- Git commits;
- release numbers;
- branch names;
- authors.

A later source-control forensic layer may explicitly bind verified repository
metadata to a case file.

## No causal inference

V1 makes no claims about:

- root cause;
- source-control blame;
- introducing commit;
- responsible author;
- likely fix;
- remediation success.

Those concepts require additional evidence and will use separate schemas so
inference is never confused with retained history fact.

## Compatibility

The meaning of `diagprint.forensics.case-file/v1` is immutable.

Backward-incompatible semantic changes require a new schema version.
