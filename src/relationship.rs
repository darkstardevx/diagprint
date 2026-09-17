use crate::{
    ArtifactDigest, Diagnostic, DiagnosticHistory, DiagnosticHistoryRun, DiagnosticReport,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const DIAGNOSTIC_RELATIONSHIP_GRAPH_V1_SCHEMA: &str = "diagprint.relationship.graph/v1";

pub const DIAGNOSTIC_RELATIONSHIP_SNAPSHOT_V1_SCHEMA: &str = "diagprint.relationship.snapshot/v1";

const RELATIONSHIP_DIRECTORY: &str = "relationships";
const QUALIFIED_FINGERPRINT_PREFIX: &str = "diagprint.canonical/v1:sha256:";
const PRODUCER_MAX_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticRelationshipKind {
    Causes,
    ContributesTo,
    DependsOn,
    DerivedFrom,
    Precedes,
    CoOccursWith,
    RelatedTo,
}

impl DiagnosticRelationshipKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Causes => "causes",
            Self::ContributesTo => "contributes_to",
            Self::DependsOn => "depends_on",
            Self::DerivedFrom => "derived_from",
            Self::Precedes => "precedes",
            Self::CoOccursWith => "co_occurs_with",
            Self::RelatedTo => "related_to",
        }
    }

    pub const fn is_causal(self) -> bool {
        matches!(self, Self::Causes | Self::ContributesTo)
    }

    pub const fn is_symmetric(self) -> bool {
        matches!(self, Self::CoOccursWith | Self::RelatedTo)
    }
}

impl fmt::Display for DiagnosticRelationshipKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticRelationshipEvidence {
    ProducerDeclared,
    SourceChain,
    Structural,
    TraceContext,
    TemporalAssociation,
    InferredCorrelation,
}

impl DiagnosticRelationshipEvidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProducerDeclared => "producer_declared",
            Self::SourceChain => "source_chain",
            Self::Structural => "structural",
            Self::TraceContext => "trace_context",
            Self::TemporalAssociation => "temporal_association",
            Self::InferredCorrelation => "inferred_correlation",
        }
    }

    pub const fn can_assert_causal_relation(self) -> bool {
        matches!(self, Self::ProducerDeclared | Self::SourceChain)
    }

    pub const fn is_inferred(self) -> bool {
        matches!(self, Self::InferredCorrelation)
    }
}

impl fmt::Display for DiagnosticRelationshipEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DiagnosticRelationshipNode {
    pub fingerprint: String,
}

impl DiagnosticRelationshipNode {
    pub fn new(fingerprint: impl Into<String>) -> Result<Self, DiagnosticRelationshipError> {
        let fingerprint = fingerprint.into();
        validate_fingerprint(&fingerprint)?;
        Ok(Self { fingerprint })
    }

    fn validate(&self) -> Result<(), DiagnosticRelationshipError> {
        validate_fingerprint(&self.fingerprint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DiagnosticRelationship {
    pub from: String,
    pub to: String,
    pub kind: DiagnosticRelationshipKind,
    pub evidence: DiagnosticRelationshipEvidence,
    pub producer: String,
}

impl DiagnosticRelationship {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        kind: DiagnosticRelationshipKind,
        evidence: DiagnosticRelationshipEvidence,
        producer: impl Into<String>,
    ) -> Result<Self, DiagnosticRelationshipError> {
        let mut from = from.into();
        let mut to = to.into();

        if kind.is_symmetric() && to < from {
            std::mem::swap(&mut from, &mut to);
        }

        let relationship = Self {
            from,
            to,
            kind,
            evidence,
            producer: producer.into(),
        };

        relationship.validate()?;
        Ok(relationship)
    }

    pub const fn is_causal(&self) -> bool {
        self.kind.is_causal()
    }

    pub const fn is_inferred(&self) -> bool {
        self.evidence.is_inferred()
    }

    fn validate(&self) -> Result<(), DiagnosticRelationshipError> {
        validate_fingerprint(&self.from)?;
        validate_fingerprint(&self.to)?;
        validate_producer(&self.producer)?;

        if self.from == self.to {
            return Err(DiagnosticRelationshipError::SelfRelationship {
                fingerprint: self.from.clone(),
            });
        }

        if self.kind.is_symmetric() && self.to < self.from {
            return Err(DiagnosticRelationshipError::NonCanonicalSymmetricEdge {
                from: self.from.clone(),
                to: self.to.clone(),
                kind: self.kind,
            });
        }

        if self.kind.is_causal() && !self.evidence.can_assert_causal_relation() {
            return Err(DiagnosticRelationshipError::CausalEvidenceMismatch {
                kind: self.kind,
                evidence: self.evidence,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DiagnosticRelationshipGraphBuilder {
    nodes: BTreeSet<DiagnosticRelationshipNode>,
    edges: BTreeSet<DiagnosticRelationship>,
}

impl DiagnosticRelationshipGraphBuilder {
    pub const fn new() -> Self {
        Self {
            nodes: BTreeSet::new(),
            edges: BTreeSet::new(),
        }
    }

    pub fn add_fingerprint(
        &mut self,
        fingerprint: impl Into<String>,
    ) -> Result<&mut Self, DiagnosticRelationshipError> {
        self.nodes
            .insert(DiagnosticRelationshipNode::new(fingerprint)?);
        Ok(self)
    }

    pub fn add_diagnostic(&mut self, diagnostic: &Diagnostic) -> &mut Self {
        self.nodes.insert(DiagnosticRelationshipNode {
            fingerprint: diagnostic.fingerprint().qualified(),
        });
        self
    }

    pub fn add_report(&mut self, report: &DiagnosticReport) -> &mut Self {
        for diagnostic in report.iter() {
            self.add_diagnostic(diagnostic);
        }
        self
    }

    pub fn add_relationship(
        &mut self,
        relationship: DiagnosticRelationship,
    ) -> Result<&mut Self, DiagnosticRelationshipError> {
        relationship.validate()?;

        self.nodes.insert(DiagnosticRelationshipNode {
            fingerprint: relationship.from.clone(),
        });
        self.nodes.insert(DiagnosticRelationshipNode {
            fingerprint: relationship.to.clone(),
        });
        self.edges.insert(relationship);

        Ok(self)
    }

    pub fn build(self) -> Result<DiagnosticRelationshipGraph, DiagnosticRelationshipError> {
        DiagnosticRelationshipGraph::from_parts(
            self.nodes.into_iter().collect(),
            self.edges.into_iter().collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRelationshipGraph {
    pub schema: String,
    pub nodes: Vec<DiagnosticRelationshipNode>,
    pub edges: Vec<DiagnosticRelationship>,
    pub graph_digest: ArtifactDigest,
}

#[derive(Serialize)]
struct RelationshipGraphDigestPayload<'a> {
    schema: &'a str,
    nodes: &'a [DiagnosticRelationshipNode],
    edges: &'a [DiagnosticRelationship],
}

impl DiagnosticRelationshipGraph {
    pub fn builder() -> DiagnosticRelationshipGraphBuilder {
        DiagnosticRelationshipGraphBuilder::new()
    }

    pub fn from_report(
        report: &DiagnosticReport,
        relationships: impl IntoIterator<Item = DiagnosticRelationship>,
    ) -> Result<Self, DiagnosticRelationshipError> {
        let mut builder = Self::builder();
        builder.add_report(report);

        for relationship in relationships {
            builder.add_relationship(relationship)?;
        }

        builder.build()
    }

    fn from_parts(
        nodes: Vec<DiagnosticRelationshipNode>,
        edges: Vec<DiagnosticRelationship>,
    ) -> Result<Self, DiagnosticRelationshipError> {
        let schema = DIAGNOSTIC_RELATIONSHIP_GRAPH_V1_SCHEMA.to_owned();

        let payload = RelationshipGraphDigestPayload {
            schema: &schema,
            nodes: &nodes,
            edges: &edges,
        };

        let graph_digest = compute_digest(&payload)?;

        let graph = Self {
            schema,
            nodes,
            edges,
            graph_digest,
        };

        graph.verify()?;
        Ok(graph)
    }

    pub fn verify(&self) -> Result<(), DiagnosticRelationshipError> {
        if self.schema != DIAGNOSTIC_RELATIONSHIP_GRAPH_V1_SCHEMA {
            return Err(DiagnosticRelationshipError::UnsupportedGraphSchema {
                schema: self.schema.clone(),
            });
        }

        validate_strict_order("graph nodes", &self.nodes)?;
        validate_strict_order("graph edges", &self.edges)?;

        let node_fingerprints = self
            .nodes
            .iter()
            .map(|node| node.fingerprint.as_str())
            .collect::<BTreeSet<_>>();

        for node in &self.nodes {
            node.validate()?;
        }

        for edge in &self.edges {
            edge.validate()?;

            if !node_fingerprints.contains(edge.from.as_str()) {
                return Err(DiagnosticRelationshipError::MissingEndpoint {
                    fingerprint: edge.from.clone(),
                });
            }

            if !node_fingerprints.contains(edge.to.as_str()) {
                return Err(DiagnosticRelationshipError::MissingEndpoint {
                    fingerprint: edge.to.clone(),
                });
            }
        }

        let payload = RelationshipGraphDigestPayload {
            schema: &self.schema,
            nodes: &self.nodes,
            edges: &self.edges,
        };

        let actual = compute_digest(&payload)?;

        if actual != self.graph_digest {
            return Err(DiagnosticRelationshipError::GraphDigestMismatch {
                expected: self.graph_digest,
                actual,
            });
        }

        Ok(())
    }

    pub fn contains(&self, fingerprint: &str) -> bool {
        self.nodes
            .binary_search_by(|node| node.fingerprint.as_str().cmp(fingerprint))
            .is_ok()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRelationshipSnapshot {
    pub schema: String,
    pub run_index: usize,
    pub history_run_digest: String,
    pub report_digest: String,
    pub graph_digest: ArtifactDigest,
    pub graph: DiagnosticRelationshipGraph,
    pub snapshot_digest: ArtifactDigest,
}

#[derive(Serialize)]
struct RelationshipSnapshotDigestPayload<'a> {
    schema: &'a str,
    run_index: usize,
    history_run_digest: &'a str,
    report_digest: &'a str,
    graph_digest: ArtifactDigest,
}

impl DiagnosticRelationshipSnapshot {
    pub fn new(
        run: &DiagnosticHistoryRun,
        graph: DiagnosticRelationshipGraph,
    ) -> Result<Self, DiagnosticRelationshipError> {
        graph.verify()?;
        validate_graph_against_run(&graph, run)?;

        let schema = DIAGNOSTIC_RELATIONSHIP_SNAPSHOT_V1_SCHEMA.to_owned();
        let history_run_digest = run.run_digest.to_string();
        let report_digest = run.report_digest.clone();
        let graph_digest = graph.graph_digest;

        let payload = RelationshipSnapshotDigestPayload {
            schema: &schema,
            run_index: run.index,
            history_run_digest: &history_run_digest,
            report_digest: &report_digest,
            graph_digest,
        };

        let snapshot_digest = compute_digest(&payload)?;

        Ok(Self {
            schema,
            run_index: run.index,
            history_run_digest,
            report_digest,
            graph_digest,
            graph,
            snapshot_digest,
        })
    }

    pub fn verify_record(&self) -> Result<(), DiagnosticRelationshipError> {
        if self.schema != DIAGNOSTIC_RELATIONSHIP_SNAPSHOT_V1_SCHEMA {
            return Err(DiagnosticRelationshipError::UnsupportedSnapshotSchema {
                schema: self.schema.clone(),
            });
        }

        self.graph.verify()?;

        if self.graph_digest != self.graph.graph_digest {
            return Err(DiagnosticRelationshipError::SnapshotGraphDigestMismatch {
                expected: self.graph_digest,
                actual: self.graph.graph_digest,
            });
        }

        let payload = RelationshipSnapshotDigestPayload {
            schema: &self.schema,
            run_index: self.run_index,
            history_run_digest: &self.history_run_digest,
            report_digest: &self.report_digest,
            graph_digest: self.graph_digest,
        };

        let actual = compute_digest(&payload)?;

        if actual != self.snapshot_digest {
            return Err(DiagnosticRelationshipError::SnapshotDigestMismatch {
                expected: self.snapshot_digest,
                actual,
            });
        }

        Ok(())
    }

    pub fn verify_against(
        &self,
        run: &DiagnosticHistoryRun,
    ) -> Result<(), DiagnosticRelationshipError> {
        self.verify_record()?;

        if self.run_index != run.index {
            return Err(DiagnosticRelationshipError::RunIndexMismatch {
                snapshot: self.run_index,
                history: run.index,
            });
        }

        let expected_run_digest = run.run_digest.to_string();

        if self.history_run_digest != expected_run_digest {
            return Err(DiagnosticRelationshipError::HistoryRunDigestMismatch {
                expected: expected_run_digest,
                actual: self.history_run_digest.clone(),
            });
        }

        if self.report_digest != run.report_digest {
            return Err(DiagnosticRelationshipError::ReportDigestMismatch {
                expected: run.report_digest.clone(),
                actual: self.report_digest.clone(),
            });
        }

        validate_graph_against_run(&self.graph, run)
    }

    pub fn persist(
        &self,
        history: &DiagnosticHistory,
    ) -> Result<PathBuf, DiagnosticRelationshipError> {
        let run = history.runs().get(self.run_index).ok_or(
            DiagnosticRelationshipError::RunOutOfRange {
                index: self.run_index,
                len: history.len(),
            },
        )?;

        self.verify_against(run)?;

        let directory = history.directory().join(RELATIONSHIP_DIRECTORY);

        fs::create_dir_all(&directory).map_err(|source| {
            io_error(
                "create diagnostic relationship directory",
                &directory,
                source,
            )
        })?;

        let path = snapshot_path_for(history, self.run_index);

        if path.exists() {
            let existing = Self::load(history, self.run_index)?
                .ok_or_else(|| DiagnosticRelationshipError::AlreadyExists { path: path.clone() })?;

            if &existing == self {
                return Ok(path);
            }

            return Err(DiagnosticRelationshipError::AlreadyExists { path });
        }

        let mut bytes =
            serde_json::to_vec_pretty(self).map_err(DiagnosticRelationshipError::Json)?;
        bytes.push(b'\n');

        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                return Err(DiagnosticRelationshipError::AlreadyExists { path });
            }
            Err(source) => {
                return Err(io_error(
                    "create diagnostic relationship snapshot",
                    &path,
                    source,
                ));
            }
        };

        file.write_all(&bytes)
            .map_err(|source| io_error("write diagnostic relationship snapshot", &path, source))?;

        file.sync_all()
            .map_err(|source| io_error("sync diagnostic relationship snapshot", &path, source))?;

        sync_directory(&directory)?;

        Ok(path)
    }

    pub fn load(
        history: &DiagnosticHistory,
        run_index: usize,
    ) -> Result<Option<Self>, DiagnosticRelationshipError> {
        let run =
            history
                .runs()
                .get(run_index)
                .ok_or(DiagnosticRelationshipError::RunOutOfRange {
                    index: run_index,
                    len: history.len(),
                })?;

        let path = snapshot_path_for(history, run_index);

        if !path.is_file() {
            return Ok(None);
        }

        let bytes = fs::read(&path)
            .map_err(|source| io_error("read diagnostic relationship snapshot", &path, source))?;

        let snapshot: Self =
            serde_json::from_slice(&bytes).map_err(DiagnosticRelationshipError::Json)?;

        snapshot.verify_against(run)?;
        Ok(Some(snapshot))
    }

    pub fn snapshot_path(history: &DiagnosticHistory, run_index: usize) -> PathBuf {
        snapshot_path_for(history, run_index)
    }
}

fn validate_graph_against_run(
    graph: &DiagnosticRelationshipGraph,
    run: &DiagnosticHistoryRun,
) -> Result<(), DiagnosticRelationshipError> {
    let observed = run
        .observations
        .iter()
        .map(|observation| observation.fingerprint.as_str())
        .collect::<BTreeSet<_>>();

    for fingerprint in &observed {
        if !graph.contains(fingerprint) {
            return Err(DiagnosticRelationshipError::MissingObservedFingerprint {
                fingerprint: (*fingerprint).to_owned(),
            });
        }
    }

    for node in &graph.nodes {
        if !observed.contains(node.fingerprint.as_str()) {
            return Err(DiagnosticRelationshipError::UnexpectedFingerprint {
                fingerprint: node.fingerprint.clone(),
            });
        }
    }

    Ok(())
}

fn validate_fingerprint(value: &str) -> Result<(), DiagnosticRelationshipError> {
    let Some(encoded) = value.strip_prefix(QUALIFIED_FINGERPRINT_PREFIX) else {
        return Err(DiagnosticRelationshipError::InvalidFingerprint {
            value: value.to_owned(),
        });
    };

    if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DiagnosticRelationshipError::InvalidFingerprint {
            value: value.to_owned(),
        });
    }

    Ok(())
}

fn validate_producer(value: &str) -> Result<(), DiagnosticRelationshipError> {
    let valid = !value.is_empty()
        && value.len() <= PRODUCER_MAX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));

    if !valid {
        return Err(DiagnosticRelationshipError::InvalidProducer {
            value: value.to_owned(),
        });
    }

    Ok(())
}

fn validate_strict_order<T: Ord>(
    field: &'static str,
    values: &[T],
) -> Result<(), DiagnosticRelationshipError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DiagnosticRelationshipError::NonCanonicalOrder { field });
    }

    Ok(())
}

fn compute_digest(value: &impl Serialize) -> Result<ArtifactDigest, DiagnosticRelationshipError> {
    let bytes = serde_json::to_vec(value).map_err(DiagnosticRelationshipError::Json)?;
    Ok(ArtifactDigest::compute(&bytes))
}

fn snapshot_path_for(history: &DiagnosticHistory, run_index: usize) -> PathBuf {
    history
        .directory()
        .join(RELATIONSHIP_DIRECTORY)
        .join(format!("run-{run_index:06}.json"))
}

fn sync_directory(path: &Path) -> Result<(), DiagnosticRelationshipError> {
    let directory = File::open(path).map_err(|source| {
        io_error(
            "open diagnostic relationship directory for sync",
            path,
            source,
        )
    })?;

    directory
        .sync_all()
        .map_err(|source| io_error("sync diagnostic relationship directory", path, source))
}

fn io_error(
    operation: &'static str,
    path: &Path,
    source: io::Error,
) -> DiagnosticRelationshipError {
    DiagnosticRelationshipError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[derive(Debug)]
pub enum DiagnosticRelationshipError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Json(serde_json::Error),
    UnsupportedGraphSchema {
        schema: String,
    },
    UnsupportedSnapshotSchema {
        schema: String,
    },
    InvalidFingerprint {
        value: String,
    },
    InvalidProducer {
        value: String,
    },
    SelfRelationship {
        fingerprint: String,
    },
    NonCanonicalSymmetricEdge {
        from: String,
        to: String,
        kind: DiagnosticRelationshipKind,
    },
    CausalEvidenceMismatch {
        kind: DiagnosticRelationshipKind,
        evidence: DiagnosticRelationshipEvidence,
    },
    NonCanonicalOrder {
        field: &'static str,
    },
    MissingEndpoint {
        fingerprint: String,
    },
    GraphDigestMismatch {
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },
    SnapshotGraphDigestMismatch {
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },
    SnapshotDigestMismatch {
        expected: ArtifactDigest,
        actual: ArtifactDigest,
    },
    RunOutOfRange {
        index: usize,
        len: usize,
    },
    RunIndexMismatch {
        snapshot: usize,
        history: usize,
    },
    HistoryRunDigestMismatch {
        expected: String,
        actual: String,
    },
    ReportDigestMismatch {
        expected: String,
        actual: String,
    },
    MissingObservedFingerprint {
        fingerprint: String,
    },
    UnexpectedFingerprint {
        fingerprint: String,
    },
    AlreadyExists {
        path: PathBuf,
    },
}

impl fmt::Display for DiagnosticRelationshipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} {}: {source}", path.display()),
            Self::Json(source) => {
                write!(formatter, "diagnostic relationship JSON error: {source}")
            }
            Self::UnsupportedGraphSchema { schema } => {
                write!(
                    formatter,
                    "unsupported diagnostic relationship graph schema {schema:?}"
                )
            }
            Self::UnsupportedSnapshotSchema { schema } => write!(
                formatter,
                "unsupported diagnostic relationship snapshot schema {schema:?}"
            ),
            Self::InvalidFingerprint { value } => {
                write!(
                    formatter,
                    "invalid qualified diagnostic fingerprint {value:?}"
                )
            }
            Self::InvalidProducer { value } => {
                write!(
                    formatter,
                    "invalid diagnostic relationship producer {value:?}"
                )
            }
            Self::SelfRelationship { fingerprint } => write!(
                formatter,
                "diagnostic relationship cannot target itself: {fingerprint}"
            ),
            Self::NonCanonicalSymmetricEdge { from, to, kind } => write!(
                formatter,
                "symmetric {kind} relationship endpoints are not canonically ordered: {from} -> {to}"
            ),
            Self::CausalEvidenceMismatch { kind, evidence } => write!(
                formatter,
                "relationship evidence {evidence} cannot assert causal relation {kind}"
            ),
            Self::NonCanonicalOrder { field } => {
                write!(formatter, "{field} are not strictly sorted and unique")
            }
            Self::MissingEndpoint { fingerprint } => write!(
                formatter,
                "diagnostic relationship endpoint is missing from graph nodes: {fingerprint}"
            ),
            Self::GraphDigestMismatch { expected, actual } => write!(
                formatter,
                "diagnostic relationship graph digest mismatch: expected {expected}, got {actual}"
            ),
            Self::SnapshotGraphDigestMismatch { expected, actual } => write!(
                formatter,
                "diagnostic relationship snapshot graph digest mismatch: expected {expected}, got {actual}"
            ),
            Self::SnapshotDigestMismatch { expected, actual } => write!(
                formatter,
                "diagnostic relationship snapshot digest mismatch: expected {expected}, got {actual}"
            ),
            Self::RunOutOfRange { index, len } => write!(
                formatter,
                "diagnostic relationship run index {index} is out of range for history containing {len} run(s)"
            ),
            Self::RunIndexMismatch { snapshot, history } => write!(
                formatter,
                "diagnostic relationship snapshot run {snapshot} does not match history run {history}"
            ),
            Self::HistoryRunDigestMismatch { expected, actual } => write!(
                formatter,
                "diagnostic relationship history-run digest mismatch: expected {expected}, got {actual}"
            ),
            Self::ReportDigestMismatch { expected, actual } => write!(
                formatter,
                "diagnostic relationship report digest mismatch: expected {expected}, got {actual}"
            ),
            Self::MissingObservedFingerprint { fingerprint } => write!(
                formatter,
                "relationship snapshot omits diagnostic observed in the bound history run: {fingerprint}"
            ),
            Self::UnexpectedFingerprint { fingerprint } => write!(
                formatter,
                "relationship snapshot contains diagnostic absent from the bound history run: {fingerprint}"
            ),
            Self::AlreadyExists { path } => write!(
                formatter,
                "diagnostic relationship snapshot already exists with different content: {}",
                path.display()
            ),
        }
    }
}

impl Error for DiagnosticRelationshipError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(source) => Some(source),
            _ => None,
        }
    }
}
