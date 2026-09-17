//! Reusable interoperability SDK for building diagprint ecosystem adapters.
//!
//! `diagprint-bridge` is intentionally ecosystem-neutral. It knows how to turn
//! normalized adapter metadata into:
//!
//! - a [`diagprint::DiagnosticReport`];
//! - a verified [`diagprint::DiagnosticRelationshipGraph`].
//!
//! Adapter crates remain responsible for understanding their upstream
//! ecosystem's structured objects and topology.
//!
//! The SDK builds on diagprint's existing [`diagprint::InteropDiagnostic`]
//! protocol instead of creating a competing diagnostic data model.
//!
//! # Identity model
//!
//! [`BridgeNodeId`] is an opaque, process-local construction handle. It exists
//! only so an adapter can describe relationships between diagnostic *instances*
//! while the bridge is being assembled.
//!
//! Durable relationship identity remains the canonical diagprint diagnostic
//! fingerprint. Node handles are never serialized or used as canonical
//! identity.

use diagprint::{
    Cause, DiagnosticRelationship, DiagnosticRelationshipError, DiagnosticRelationshipEvidence,
    DiagnosticRelationshipGraph, DiagnosticRelationshipKind, DiagnosticReport, DocumentationLink,
    IDENTITY_ATTRIBUTE, InteropDiagnostic, InteropLabel, Reporter, Severity,
};
use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_IDENTITY_LEN: usize = 256;
const MAX_PRODUCER_LEN: usize = 64;

static NEXT_BUILDER_SCOPE: AtomicU64 = AtomicU64::new(1);

/// Reusable mapper output shared by ecosystem adapters.
///
/// The actual diagnostic payload is diagprint's dependency-free
/// [`InteropDiagnostic`]. The bridge SDK only adds the optional
/// application-owned logical identity needed by canonical fingerprinting.
#[derive(Debug, Clone)]
pub struct BridgeDiagnosticMetadata {
    diagnostic: InteropDiagnostic,
    identity: Option<String>,
}

impl BridgeDiagnosticMetadata {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            diagnostic: InteropDiagnostic::new(message),
            identity: None,
        }
    }

    pub fn from_interop(diagnostic: InteropDiagnostic) -> Self {
        Self {
            diagnostic,
            identity: None,
        }
    }

    pub fn severity(mut self, severity: Severity) -> Self {
        self.diagnostic = self.diagnostic.severity(severity);
        self
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.code(code);
        self
    }

    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.help(help);
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.note(note);
        self
    }

    pub fn label(mut self, label: InteropLabel) -> Self {
        self.diagnostic = self.diagnostic.label(label);
        self
    }

    pub fn cause(mut self, cause: Cause) -> Self {
        self.diagnostic = self.diagnostic.cause(cause);
        self
    }

    pub fn documentation(mut self, link: DocumentationLink) -> Self {
        self.diagnostic = self.diagnostic.documentation(link);
        self
    }

    pub fn related(mut self, diagnostic: InteropDiagnostic) -> Self {
        self.diagnostic = self.diagnostic.related(diagnostic);
        self
    }

    /// Sets an application-owned stable logical identity.
    ///
    /// The SDK intentionally allows applications to choose their own namespace.
    /// It only rejects values which are empty, unreasonably large, or contain
    /// ASCII control characters.
    pub fn identity(mut self, identity: impl Into<String>) -> Result<Self, BridgeError> {
        let identity = identity.into();
        validate_identity(&identity)?;
        self.identity = Some(identity);
        Ok(self)
    }

    pub fn interop(&self) -> &InteropDiagnostic {
        &self.diagnostic
    }

    pub fn identity_value(&self) -> Option<&str> {
        self.identity.as_deref()
    }

    fn into_diagprint(self, reporter: &Reporter) -> diagprint::Diagnostic {
        let mut diagnostic = self.diagnostic.to_diagprint(reporter);

        if let Some(identity) = self.identity {
            diagnostic = diagnostic.attribute(IDENTITY_ATTRIBUTE, identity);
        }

        diagnostic
    }
}

/// Opaque instance handle used only while constructing one bridge output.
///
/// Handles are scoped to the [`BridgeOutputBuilder`] which created them. A
/// handle from another builder is rejected instead of being interpreted as a
/// coincidentally matching index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BridgeNodeId {
    scope: u64,
    index: usize,
}

/// Generic bridge assembly statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeBuildStats {
    pub diagnostic_instances: usize,
    pub logical_nodes: usize,
    pub relationships: usize,
    pub collapsed_self_relationships: usize,
}

/// Completed ecosystem bridge output.
#[derive(Debug)]
pub struct BridgeOutput {
    report: DiagnosticReport,
    graph: DiagnosticRelationshipGraph,
    stats: BridgeBuildStats,
}

impl BridgeOutput {
    pub fn report(&self) -> &DiagnosticReport {
        &self.report
    }

    pub fn graph(&self) -> &DiagnosticRelationshipGraph {
        &self.graph
    }

    pub const fn stats(&self) -> BridgeBuildStats {
        self.stats
    }

    pub fn into_parts(
        self,
    ) -> (
        DiagnosticReport,
        DiagnosticRelationshipGraph,
        BridgeBuildStats,
    ) {
        (self.report, self.graph, self.stats)
    }
}

/// Reusable builder for adapter-produced diagnostics and relationships.
///
/// One builder corresponds to one conversion operation.
pub struct BridgeOutputBuilder<'a> {
    reporter: &'a Reporter,
    producer: String,
    scope: u64,
    report: DiagnosticReport,
    fingerprints: Vec<String>,
    relationships: Vec<DiagnosticRelationship>,
    collapsed_self_relationships: usize,
}

impl<'a> BridgeOutputBuilder<'a> {
    pub fn new(reporter: &'a Reporter, producer: impl Into<String>) -> Result<Self, BridgeError> {
        let producer = producer.into();
        validate_producer(&producer)?;

        Ok(Self {
            reporter,
            producer,
            scope: next_builder_scope(),
            report: DiagnosticReport::new(),
            fingerprints: Vec::new(),
            relationships: Vec::new(),
            collapsed_self_relationships: 0,
        })
    }

    pub fn producer(&self) -> &str {
        &self.producer
    }

    /// Appends one diagnostic instance and returns an ephemeral construction
    /// handle for relating it to other instances.
    pub fn push(
        &mut self,
        metadata: BridgeDiagnosticMetadata,
    ) -> Result<BridgeNodeId, BridgeError> {
        if let Some(identity) = metadata.identity_value() {
            validate_identity(identity)?;
        }

        let diagnostic = metadata.into_diagprint(self.reporter);
        let fingerprint = diagnostic.fingerprint().qualified();

        let index = self.fingerprints.len();
        self.fingerprints.push(fingerprint);
        self.report.push(diagnostic);

        Ok(BridgeNodeId {
            scope: self.scope,
            index,
        })
    }

    /// Adds one logical relationship between two diagnostic instances.
    ///
    /// If the two instances resolve to the same logical fingerprint, no invalid
    /// M4 self-edge is invented. The collapse is recorded in build statistics.
    pub fn relate(
        &mut self,
        from: BridgeNodeId,
        to: BridgeNodeId,
        kind: DiagnosticRelationshipKind,
        evidence: DiagnosticRelationshipEvidence,
    ) -> Result<&mut Self, BridgeError> {
        let from_fingerprint = self.resolve(from)?.to_owned();
        let to_fingerprint = self.resolve(to)?.to_owned();

        if from_fingerprint == to_fingerprint {
            self.collapsed_self_relationships = self.collapsed_self_relationships.saturating_add(1);
            return Ok(self);
        }

        self.relationships.push(DiagnosticRelationship::new(
            from_fingerprint,
            to_fingerprint,
            kind,
            evidence,
            self.producer.clone(),
        )?);

        Ok(self)
    }

    /// Completes and verifies the deterministic M4 relationship graph.
    pub fn finish(self) -> Result<BridgeOutput, BridgeError> {
        let graph = DiagnosticRelationshipGraph::from_report(&self.report, self.relationships)?;

        let stats = BridgeBuildStats {
            diagnostic_instances: self.report.len(),
            logical_nodes: graph.node_count(),
            relationships: graph.edge_count(),
            collapsed_self_relationships: self.collapsed_self_relationships,
        };

        Ok(BridgeOutput {
            report: self.report,
            graph,
            stats,
        })
    }

    fn resolve(&self, node: BridgeNodeId) -> Result<&str, BridgeError> {
        if node.scope != self.scope {
            return Err(BridgeError::ForeignNode {
                expected_scope: self.scope,
                actual_scope: node.scope,
            });
        }

        self.fingerprints
            .get(node.index)
            .map(String::as_str)
            .ok_or(BridgeError::UnknownNode {
                index: node.index,
                len: self.fingerprints.len(),
            })
    }
}

fn next_builder_scope() -> u64 {
    loop {
        let scope = NEXT_BUILDER_SCOPE.fetch_add(1, Ordering::Relaxed);

        if scope != 0 {
            return scope;
        }
    }
}

fn validate_identity(value: &str) -> Result<(), BridgeError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_IDENTITY_LEN
        && !value.chars().any(char::is_control);

    if valid {
        Ok(())
    } else {
        Err(BridgeError::InvalidIdentity {
            value: value.to_owned(),
        })
    }
}

fn validate_producer(value: &str) -> Result<(), BridgeError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_PRODUCER_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));

    if valid {
        Ok(())
    } else {
        Err(BridgeError::InvalidProducer {
            value: value.to_owned(),
        })
    }
}

#[derive(Debug)]
pub enum BridgeError {
    InvalidIdentity {
        value: String,
    },
    InvalidProducer {
        value: String,
    },
    ForeignNode {
        expected_scope: u64,
        actual_scope: u64,
    },
    UnknownNode {
        index: usize,
        len: usize,
    },
    Relationship(DiagnosticRelationshipError),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity { value } => {
                write!(formatter, "invalid bridge logical identity {value:?}")
            }
            Self::InvalidProducer { value } => {
                write!(formatter, "invalid bridge producer {value:?}")
            }
            Self::ForeignNode {
                expected_scope,
                actual_scope,
            } => write!(
                formatter,
                "bridge node belongs to another builder scope: expected {expected_scope}, got {actual_scope}"
            ),
            Self::UnknownNode { index, len } => write!(
                formatter,
                "bridge node index {index} is out of range for {len} diagnostic instance(s)"
            ),
            Self::Relationship(source) => {
                write!(
                    formatter,
                    "bridge relationship construction failed: {source}"
                )
            }
        }
    }
}

impl Error for BridgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Relationship(source) => Some(source),
            _ => None,
        }
    }
}

impl From<DiagnosticRelationshipError> for BridgeError {
    fn from(source: DiagnosticRelationshipError) -> Self {
        Self::Relationship(source)
    }
}
