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

## Traversal and cascade analysis

M4B adds cycle-safe depth-bounded traversal.

Directions:

- `upstream`
- `downstream`
- `both`

Evidence filters:

- `explicit` — excludes `inferred_correlation`;
- `all` — includes every retained evidence class.

Traversal uses visited sets and never assumes the complete graph is acyclic.

Explicit causal cascade helpers traverse only the `causes` and
`contributes_to` relation kinds.

Their results describe recorded explicit causal-edge topology. They are not an
independent root-cause determination.

## CLI

Top-level:

`diagprint graph <HISTORY> <FINGERPRINT> [OPTIONS]`

History alias:

`diagprint history graph <HISTORY> <FINGERPRINT> [OPTIONS]`

Options:

- `--run <N>`
- `--depth <N>`
- `--direction upstream|downstream|both`
- `--evidence explicit|all`
- `--format text|json|dot`

If `--run` is omitted, diagprint selects the newest verified relationship
snapshot containing the requested diagnostic fingerprint.

The CLI re-verifies diagnostic history before loading relationship evidence.

The default evidence filter is `explicit`, so inferred correlations must be
requested deliberately with `--evidence all`.

### Text format

Text output separates:

- explicit producer/source-chain relationships;
- structural/trace relationships;
- temporal associations;
- inferred correlations.

It also reports explicit causal upstream/downstream cascade topology and states
that independent root cause is not established.

### JSON format

JSON preserves:

- exact history bindings;
- snapshot and graph digests;
- traversal parameters;
- explicit causal upstream/downstream sets;
- the deterministic subgraph.

### DOT format

DOT output is deterministic Graphviz text.

Graphviz is not a diagprint dependency.

Every edge label retains:

`relationship kind / evidence provenance / producer`

Symmetric relationship kinds are emitted with bidirectional DOT presentation.
