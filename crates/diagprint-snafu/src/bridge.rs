use crate::{
    metadata::SnafuDiagnosticMetadata,
    policy::{SnafuBacktracePolicy, SnafuCaptureProfile, SnafuTextPolicy},
};
use diagprint::{
    DiagnosticRelationshipEvidence, DiagnosticRelationshipGraph, DiagnosticRelationshipKind,
    DiagnosticReport, Reporter,
};
use diagprint_bridge::{
    BridgeBuildStats, BridgeDiagnosticMetadata, BridgeError, BridgeOutput, BridgeOutputBuilder,
};
use snafu::{ErrorCompat, Whatever, WhateverLocal};
use std::{any::TypeId, error::Error as StdError, fmt};

const PRODUCER: &str = "snafu";
const MAX_SOURCE_DEPTH: usize = 128;
const REDACTED_UNMAPPED_SOURCE: &str = "[redacted unmapped SNAFU source]";

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

/// Closure-friendly mapper adapter.
pub struct SnafuMapperFn<F> {
    mapper: F,
}

impl<F> SnafuMapperFn<F> {
    pub const fn new(mapper: F) -> Self {
        Self { mapper }
    }

    pub fn into_inner(self) -> F {
        self.mapper
    }
}

impl<F> SnafuErrorMapper for SnafuMapperFn<F>
where
    F: for<'a> Fn(SnafuErrorView<'a>) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError>,
{
    fn map(
        &self,
        view: SnafuErrorView<'_>,
    ) -> Result<Option<SnafuDiagnosticMetadata>, SnafuBridgeError> {
        (self.mapper)(view)
    }
}

pub const fn snafu_mapper<F>(mapper: F) -> SnafuMapperFn<F> {
    SnafuMapperFn::new(mapper)
}

#[derive(Debug)]
pub struct SnafuBridgeOutput {
    bridge: BridgeOutput,
    source_nodes: usize,
    mapped_nodes: usize,
    unmapped_nodes: usize,
    redacted_nodes: usize,
    backtraces_included: usize,
    whatever_nodes: usize,
    whatever_local_nodes: usize,
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

    pub const fn redacted_nodes(&self) -> usize {
        self.redacted_nodes
    }

    pub const fn backtraces_included(&self) -> usize {
        self.backtraces_included
    }

    pub const fn whatever_nodes(&self) -> usize {
        self.whatever_nodes
    }

    pub const fn whatever_local_nodes(&self) -> usize {
        self.whatever_local_nodes
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
        self.convert_with_profile(error, reporter, &SnafuCaptureProfile::default())
    }

    pub fn convert_with_profile<E, M>(
        &self,
        error: &E,
        reporter: &Reporter,
        profile: &SnafuCaptureProfile<M>,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError>
    where
        E: SnafuDiagnostic,
        M: SnafuErrorMapper,
    {
        let root = error.diagprint_metadata()?;
        self.convert_with_root_metadata(error, reporter, root, profile)
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
        let profile = SnafuCaptureProfile::new(mapper);
        self.convert_mapped_with_profile(error, reporter, &profile)
    }

    pub fn convert_mapped_with_profile<E, M>(
        &self,
        error: &E,
        reporter: &Reporter,
        profile: &SnafuCaptureProfile<M>,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper,
    {
        let root_error: &(dyn StdError + 'static) = error;
        let root = profile
            .mapper()
            .map(SnafuErrorView {
                error: root_error,
                depth: 0,
                is_root: true,
            })?
            .ok_or(SnafuBridgeError::MissingRootIdentity)?;

        self.convert_with_root_metadata(error, reporter, root, profile)
    }

    /// Convert an error with application-supplied stable root metadata.
    ///
    /// This is the low-level path used by the strong Whatever integration and
    /// is also useful for foreign error types whose root identity is known by
    /// the caller.
    pub fn convert_with_metadata<E, M>(
        &self,
        error: &E,
        reporter: &Reporter,
        root_metadata: SnafuDiagnosticMetadata,
        profile: &SnafuCaptureProfile<M>,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper,
    {
        self.convert_with_root_metadata(error, reporter, root_metadata, profile)
    }

    fn convert_with_root_metadata<E, M>(
        &self,
        error: &E,
        reporter: &Reporter,
        mut root_metadata: SnafuDiagnosticMetadata,
        profile: &SnafuCaptureProfile<M>,
    ) -> Result<SnafuBridgeOutput, SnafuBridgeError>
    where
        E: StdError + ErrorCompat + 'static,
        M: SnafuErrorMapper,
    {
        let mut backtraces_included = 0usize;

        if profile.backtrace_policy_value() == SnafuBacktracePolicy::DisplayText {
            if let Some(backtrace) = ErrorCompat::backtrace(error) {
                root_metadata = root_metadata.note(format!("SNAFU backtrace:\n{backtrace}"));
                backtraces_included = 1;
            }
        }

        let mut builder = BridgeOutputBuilder::new(reporter, PRODUCER)?;
        let root_error: &(dyn StdError + 'static) = error;
        let root_node = builder.push(root_metadata.into_bridge_metadata()?)?;

        let root_data = root_error as *const (dyn StdError + 'static) as *const ();
        let mut seen: Vec<*const (dyn StdError + 'static)> =
            vec![root_error as *const (dyn StdError + 'static)];

        let mut wrapper = root_node;
        let mut current = root_error.source();
        let mut depth = 1usize;
        let mut source_nodes = 0usize;
        let mut mapped_nodes = 1usize;
        let mut unmapped_nodes = 0usize;
        let mut redacted_nodes = 0usize;

        while let Some(source) = current {
            if depth > MAX_SOURCE_DEPTH {
                return Err(SnafuBridgeError::SourceDepthExceeded {
                    limit: MAX_SOURCE_DEPTH,
                });
            }

            let source_pointer = source as *const (dyn StdError + 'static);
            let source_data = source_pointer as *const ();

            let revisits_root = source_data == root_data && source.is::<E>();
            let revisits_seen_pointer = seen
                .iter()
                .any(|seen_pointer| std::ptr::eq(*seen_pointer, source_pointer));

            if revisits_root || revisits_seen_pointer {
                return Err(SnafuBridgeError::SourceCycle { depth });
            }

            seen.push(source_pointer);

            let metadata = match profile.mapper().map(SnafuErrorView {
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
                    let message = match profile.text_policy_value() {
                        SnafuTextPolicy::Display => source.to_string(),
                        SnafuTextPolicy::RedactUnmapped => {
                            redacted_nodes = redacted_nodes.saturating_add(1);
                            REDACTED_UNMAPPED_SOURCE.to_owned()
                        }
                    };
                    BridgeDiagnosticMetadata::new(message)
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

        let type_id = TypeId::of::<E>();
        let whatever_nodes = usize::from(type_id == TypeId::of::<Whatever>());
        let whatever_local_nodes = usize::from(type_id == TypeId::of::<WhateverLocal>());

        Ok(SnafuBridgeOutput {
            bridge: builder.finish()?,
            source_nodes,
            mapped_nodes,
            unmapped_nodes,
            redacted_nodes,
            backtraces_included,
            whatever_nodes,
            whatever_local_nodes,
        })
    }
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
