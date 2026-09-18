use crate::metadata::SnafuDiagnosticMetadata;
use diagprint::{
    DiagnosticRelationshipEvidence, DiagnosticRelationshipGraph, DiagnosticRelationshipKind,
    DiagnosticReport, Reporter,
};
use diagprint_bridge::{
    BridgeBuildStats, BridgeDiagnosticMetadata, BridgeError, BridgeOutput, BridgeOutputBuilder,
};
use snafu::ErrorCompat;
use std::{collections::BTreeSet, error::Error as StdError, fmt};

const PRODUCER: &str = "snafu";
const MAX_SOURCE_DEPTH: usize = 128;

pub trait SnafuDiagnostic: StdError + ErrorCompat + 'static {
    fn diagprint_metadata(&self) -> Result<SnafuDiagnosticMetadata, SnafuBridgeError>;
}

pub struct SnafuErrorView<'a> {
    error: &'a (dyn StdError + 'static),
    depth: usize,
    is_root: bool,
}
impl<'a> SnafuErrorView<'a> {
    pub const fn error(&self) -> &'a (dyn StdError + 'static) {
        self.error
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub const fn is_root(&self) -> bool {
        self.is_root
    }
    pub fn downcast_ref<T: StdError + 'static>(&self) -> Option<&T> {
        self.error.downcast_ref::<T>()
    }
}

pub trait SnafuErrorMapper {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError>;
}
impl<T: SnafuErrorMapper + ?Sized> SnafuErrorMapper for &T {
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        (**self).map(view)
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultSnafuErrorMapper;
impl SnafuErrorMapper for DefaultSnafuErrorMapper {
    fn map(
        &self,
        _view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        Ok(None)
    }
}

#[derive(Debug)]
pub struct SnafuBridgeOutput {
    bridge: BridgeOutput,
    source_nodes: usize,
    mapped_nodes: usize,
    unmapped_nodes: usize,
}
impl SnafuBridgeOutput {
    pub fn bridge(&self) -> &BridgeOutput {
        &self.bridge
    }
    pub fn report(&self) -> &DiagnosticReport {
        self.bridge.report()
    }
    pub fn graph(&self) -> &DiagnosticRelationshipGraph {
        self.bridge.graph()
    }
    pub const fn stats(&self) -> BridgeBuildStats {
        self.bridge.stats()
    }
    pub const fn source_nodes(&self) -> usize {
        self.source_nodes
    }
    pub const fn source_relationships(&self) -> usize {
        self.bridge.stats().relationships
    }
    pub const fn mapped_nodes(&self) -> usize {
        self.mapped_nodes
    }
    pub const fn unmapped_nodes(&self) -> usize {
        self.unmapped_nodes
    }
    pub fn into_bridge(self) -> BridgeOutput {
        self.bridge
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SnafuBridge;
impl SnafuBridge {
    pub const fn new() -> Self {
        Self
    }
    pub fn convert<E: SnafuDiagnostic>(
        &self,
        error: &E,
        reporter: &Reporter,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError> {
        let root = error.diagprint_metadata()?;
        self.convert_with_root_metadata(error, reporter, root, &DefaultSnafuErrorMapper)
    }
    pub fn convert_with_mapper<E, M>(
        &self,
        error: &E,
        reporter: &Reporter,
        mapper: &M,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        let root_error: &(dyn StdError + 'static) = error;
        let root = mapper
            .map(SnafuErrorView {
                error: root_error,
                depth: 0,
                is_root: true,
            })?
            .ok_or(SnafuBridgeError::MissingRootIdentity)?;
        self.convert_with_root_metadata(error, reporter, root, mapper)
    }
    fn convert_with_root_metadata<E, M>(
        &self,
        error: &E,
        reporter: &Reporter,
        root_metadata: SnafuDiagnosticMetadata,
        mapper: &M,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper + ?Sized,
    {
        let mut builder = BridgeOutputBuilder::new(reporter, PRODUCER)?;
        let root_error: &(dyn StdError + 'static) = error;
        let root_node = builder.push(root_metadata.into_bridge_metadata()?)?;
        let mut seen = BTreeSet::new();
        seen.insert(error_address(root_error));
        let mut wrapper = root_node;
        let mut current = root_error.source();
        let mut depth = 1usize;
        let mut source_nodes = 0usize;
        let mut mapped_nodes = 1usize;
        let mut unmapped_nodes = 0usize;
        while let Some(source) = current {
            if depth > MAX_SOURCE_DEPTH {
                return Err(SnafuBridgeError::SourceDepthExceeded {
                    limit: MAX_SOURCE_DEPTH,
                });
            }
            if !seen.insert(error_address(source)) {
                return Err(SnafuBridgeError::SourceCycle { depth });
            }
            let metadata = match mapper.map(SnafuErrorView {
                error: source,
                depth,
                is_root: false,
            })? {
                Some(metadata) => {
                    mapped_nodes = mapped_nodes.saturating_add(1);
                    metadata.into_bridge_metadata()?
                }
                None => {
                    unmapped_nodes = unmapped_nodes.saturating_add(1);
                    BridgeDiagnosticMetadata::new(source.to_string())
                }
            };
            let source_node = builder.push(metadata)?;
            builder.relate(
                source_node,
                wrapper,
                DiagnosticRelationshipKind::ContributesTo,
                DiagnosticRelationshipEvidence::SourceChain,
            )?;
            source_nodes = source_nodes.saturating_add(1);
            wrapper = source_node;
            current = source.source();
            depth = depth.saturating_add(1);
        }
        Ok(SnafuBridgeOutput {
            bridge: builder.finish()?,
            source_nodes,
            mapped_nodes,
            unmapped_nodes,
        })
    }
}
fn error_address(error: &(dyn StdError + 'static)) -> usize {
    let p = error as *const dyn StdError;
    p as *const () as usize
}

#[derive(Debug)]
pub enum SnafuBridgeError {
    Bridge(BridgeError),
    InvalidIdentity,
    InvalidCode,
    MissingRootIdentity,
    SourceCycle { depth: usize },
    SourceDepthExceeded { limit: usize },
}
impl fmt::Display for SnafuBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(e) => write!(f, "SNAFU bridge conversion failed: {e}"),
            Self::InvalidIdentity => f.write_str("invalid stable SNAFU diagnostic identity"),
            Self::InvalidCode => f.write_str("invalid stable SNAFU diagnostic code"),
            Self::MissingRootIdentity => {
                f.write_str("SNAFU diagnostic capture requires stable root identity")
            }
            Self::SourceCycle { depth } => {
                write!(f, "SNAFU source chain contains a cycle at depth {depth}")
            }
            Self::SourceDepthExceeded { limit } => write!(
                f,
                "SNAFU source chain exceeds the defensive depth limit of {limit}"
            ),
        }
    }
}
impl StdError for SnafuBridgeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Bridge(e) => Some(e),
            _ => None,
        }
    }
}
impl From<BridgeError> for SnafuBridgeError {
    fn from(e: BridgeError) -> Self {
        Self::Bridge(e)
    }
}
