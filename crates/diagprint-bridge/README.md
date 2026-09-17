# diagprint-bridge

Reusable interoperability SDK for building structured
[diagprint](https://crates.io/crates/diagprint) ecosystem adapters.

`diagprint-bridge` is intentionally **not** an adapter for one particular error
crate. It is the common construction layer used by adapters such as
`diagprint-error-stack` and future SNAFU, eyre, tracing-error, compiler, or
tooling bridges.

## Design

The SDK builds on two existing diagprint contracts:

- `InteropDiagnostic` for normalized diagnostic payloads;
- `DiagnosticRelationshipGraph` for durable logical relationships.

It does not replace either one.

An adapter typically does this:

```text
upstream structured object
        |
        v
adapter-specific mapper
        |
        v
BridgeDiagnosticMetadata
        |
        v
BridgeOutputBuilder
     |       |
     v       v
Diagnostic  M4 relationship
Report      graph
```

## Example

```rust
use diagprint::{
    DiagnosticRelationshipEvidence, DiagnosticRelationshipKind, Reporter,
};
use diagprint_bridge::{BridgeDiagnosticMetadata, BridgeOutputBuilder};

let reporter = Reporter::builder()
    .application("my-adapter")
    .build()?;

let mut bridge = BridgeOutputBuilder::new(&reporter, "my-adapter")?;

let source = bridge.push(
    BridgeDiagnosticMetadata::new("low-level failure")
        .identity("demo.low-level")?,
)?;

let outer = bridge.push(
    BridgeDiagnosticMetadata::new("operation failed")
        .identity("demo.operation")?,
)?;

bridge.relate(
    source,
    outer,
    DiagnosticRelationshipKind::ContributesTo,
    DiagnosticRelationshipEvidence::SourceChain,
)?;

let output = bridge.finish()?;

assert_eq!(output.report().len(), 2);
assert_eq!(output.graph().edge_count(), 1);

# Ok::<(), Box<dyn std::error::Error>>(())
```

## BridgeNodeId is not identity

`BridgeNodeId` is an opaque construction handle scoped to one
`BridgeOutputBuilder`.

It exists so adapters can describe relationships between source instances before
those instances have been reduced to canonical logical fingerprints.

It is intentionally not serialized and must never be treated as durable
diagnostic identity.

Durable graph identity remains diagprint's canonical diagnostic fingerprint.

## Duplicate logical diagnostics

Two source instances may intentionally map to one logical diagnostic identity.

The SDK preserves both instances in `DiagnosticReport`, while the M4 graph keeps
one logical node.

If an adapter relates those two instances, the SDK does not invent an invalid
logical self-edge. Instead it records a collapsed-self relationship count in
`BridgeBuildStats`.

## Mapper boundary

The SDK owns reusable mapper **output** through `BridgeDiagnosticMetadata`.

Adapter input remains ecosystem-specific. For example:

```text
ErrorStackContextView -> BridgeDiagnosticMetadata
SnafuContextView      -> BridgeDiagnosticMetadata
EyreContextView       -> BridgeDiagnosticMetadata
```

This keeps common lifecycle mechanics reusable without forcing unrelated error
ecosystems into one artificial source trait.

## Privacy

The SDK does not automatically capture:

- memory addresses;
- `TypeId`;
- environment values;
- source files;
- backtraces;
- span traces;
- arbitrary debug renderings.

Adapter crates define their own upstream content policy.

## MSRV

`diagprint-bridge` follows the diagprint workspace MSRV: Rust 1.85.
