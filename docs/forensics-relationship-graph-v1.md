# Diagnostic relationship graph v1

M4 introduces a separate typed relationship layer for canonical diagnostic
identities.

The relationship graph does not modify `Diagnostic`, canonical-v1 identity, or
history-v2.

## Schemas

- `diagprint.relationship.graph/v1`
- `diagprint.relationship.snapshot/v1`

## Identity

Nodes use full qualified `DiagnosticFingerprint` values from
`diagprint.canonical/v1`.

Graph identity is the SHA-256 digest of deterministic compact JSON containing
the schema, sorted unique nodes, and sorted unique edges.

Duplicate-identical edges collapse before graph construction.

## Relationship semantics

Current semantic kinds:

- `causes`
- `contributes_to`
- `depends_on`
- `derived_from`
- `precedes`
- `co_occurs_with`
- `related_to`

Semantics are separate from evidence provenance.

## Evidence provenance

Current evidence classes:

- `producer_declared`
- `source_chain`
- `structural`
- `trace_context`
- `temporal_association`
- `inferred_correlation`

Only producer-declared and source-chain evidence may assert the explicitly
causal `causes` and `contributes_to` relationship kinds.

This is a representation rule, not a claim that every producer-declared edge
has been independently proven.

Temporal, tracing, structural, and inferred evidence cannot silently promote
themselves into causal claims.

## Symmetric relationships

`co_occurs_with` and `related_to` are symmetric.

Their endpoints are normalized into lexical order so A/B and B/A have one
stable identity.

## History binding

Relationship snapshots are stored separately from history-v2 under:

`<HISTORY>/relationships/run-XXXXXX.json`

A snapshot binds the run index, history run digest, report digest, graph digest,
and its own snapshot digest.

The graph node set must exactly match fingerprints observed in the bound history
run.

Identical persistence is idempotent. Conflicting replacement is rejected.

## Privacy

Relationship snapshots retain canonical fingerprints, typed relationship
metadata, bounded producer identifiers, and cryptographic bindings.

They do not retain diagnostic messages, source text, source paths, arbitrary
attributes, help, notes, remediation contents, Git author identity, remotes, or
absolute repository paths.

## Ecosystem Bridges

The relationship API is the common substrate future bridge crates use to
preserve structured upstream relationships instead of flattening them to
rendered strings.

M4A supplies the graph and persistence foundation.

Traversal, cascade analysis, and CLI text/JSON/DOT presentation follow in M4B.
