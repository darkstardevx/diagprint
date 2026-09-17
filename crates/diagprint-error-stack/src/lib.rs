//! Structured [`error_stack`] interoperability for diagprint.
//!
//! This crate is the first ecosystem adapter built on
//! [`diagprint_bridge`]. It keeps `error-stack`-specific frame traversal here
//! while delegating normalized diagnostic/report/relationship assembly to the
//! reusable bridge SDK.
//!
//! E1A supports stable single-context [`error_stack::Report<C>`] conversion.
//! Grouped `Report<[C]>` conversion and explicit attachment-content policy are
//! added in E1B.
//!
//! # Structural conversion
//!
//! The adapter uses [`error_stack::Frame::sources`] and
//! [`error_stack::Frame::kind`] directly. It never parses the rendered
//! `Display` or `Debug` representation of a report.
//!
//! Context frames become diagnostic instances. Attachment frames are
//! traversal-transparent in E1A: their content is not read or exported, but
//! their source edges are followed so context topology is preserved.

use diagprint::{
    DiagnosticRelationshipEvidence, DiagnosticRelationshipGraph, DiagnosticRelationshipKind,
    DiagnosticReport, Reporter,
};
use diagprint_bridge::{
    BridgeBuildStats, BridgeDiagnosticMetadata, BridgeError, BridgeNodeId, BridgeOutput,
    BridgeOutputBuilder,
};
use error_stack::{Frame, FrameKind, Report};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

const PRODUCER: &str = "error-stack";

/// Structured `error-stack` context exposed to application mappers.
///
/// `depth` is presentation/traversal metadata only. It is never used as
/// canonical identity.
pub struct ErrorStackContextView<'a> {
    frame: &'a Frame,
    context: &'a (dyn Error + Send + Sync + 'static),
    depth: usize,
}

impl<'a> ErrorStackContextView<'a> {
    pub const fn frame(&self) -> &'a Frame {
        self.frame
    }

    pub const fn context(&self) -> &'a (dyn Error + Send + Sync + 'static) {
        self.context
    }

    pub const fn depth(&self) -> usize {
        self.depth
    }
}

/// Maps one structured `error-stack` context into reusable bridge metadata.
///
/// Implementations can use [`Frame::downcast_ref`] to inspect known context
/// types and return [`BridgeDiagnosticMetadata`] with domain-specific code,
/// severity, help, notes, or logical identity.
pub trait ErrorStackContextMapper {
    fn map(&self, view: ErrorStackContextView<'_>)
    -> Result<BridgeDiagnosticMetadata, BridgeError>;
}

impl<T> ErrorStackContextMapper for &T
where
    T: ErrorStackContextMapper + ?Sized,
{
    fn map(
        &self,
        view: ErrorStackContextView<'_>,
    ) -> Result<BridgeDiagnosticMetadata, BridgeError> {
        (**self).map(view)
    }
}

/// Default mapping for arbitrary stable `error-stack` contexts.
///
/// The context object's own `Display` text becomes the diagnostic message.
/// No synthetic external logical identity is invented.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultErrorStackContextMapper;

impl ErrorStackContextMapper for DefaultErrorStackContextMapper {
    fn map(
        &self,
        view: ErrorStackContextView<'_>,
    ) -> Result<BridgeDiagnosticMetadata, BridgeError> {
        Ok(BridgeDiagnosticMetadata::new(view.context().to_string()))
    }
}

/// Completed `error-stack` conversion.
///
/// Adapter-specific frame counts remain here while the generic report, graph,
/// and bridge statistics come from [`BridgeOutput`].
#[derive(Debug)]
pub struct ErrorStackBridgeOutput {
    bridge: BridgeOutput,
    context_frames: usize,
    attachment_frames: usize,
}

impl ErrorStackBridgeOutput {
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

    pub const fn context_frames(&self) -> usize {
        self.context_frames
    }

    pub const fn attachment_frames(&self) -> usize {
        self.attachment_frames
    }

    pub fn into_bridge(self) -> BridgeOutput {
        self.bridge
    }
}

/// Converts stable `error-stack` structure through the reusable bridge SDK.
#[derive(Debug, Clone)]
pub struct ErrorStackBridge<M = DefaultErrorStackContextMapper> {
    mapper: M,
}

impl ErrorStackBridge<DefaultErrorStackContextMapper> {
    pub const fn new() -> Self {
        Self {
            mapper: DefaultErrorStackContextMapper,
        }
    }
}

impl Default for ErrorStackBridge<DefaultErrorStackContextMapper> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> ErrorStackBridge<M>
where
    M: ErrorStackContextMapper,
{
    pub const fn with_mapper(mapper: M) -> Self {
        Self { mapper }
    }

    /// Converts one single-context `error-stack::Report<C>`.
    pub fn convert<C>(
        &self,
        report: &Report<C>,
        reporter: &Reporter,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        C: Error + Send + Sync + 'static,
    {
        let mut traversal = Traversal::new(&self.mapper, reporter)?;
        traversal.walk(report.current_frame(), 0)?;
        traversal.finish()
    }
}

/// Convenience conversion methods for single-context reports.
pub trait ErrorStackReportExt {
    fn to_diagprint(
        &self,
        reporter: &Reporter,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>;

    fn to_diagprint_with<M>(
        &self,
        reporter: &Reporter,
        mapper: &M,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        M: ErrorStackContextMapper + ?Sized;
}

impl<C> ErrorStackReportExt for Report<C>
where
    C: Error + Send + Sync + 'static,
{
    fn to_diagprint(
        &self,
        reporter: &Reporter,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError> {
        ErrorStackBridge::new().convert(self, reporter)
    }

    fn to_diagprint_with<M>(
        &self,
        reporter: &Reporter,
        mapper: &M,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        M: ErrorStackContextMapper + ?Sized,
    {
        ErrorStackBridge::with_mapper(mapper).convert(self, reporter)
    }
}

#[derive(Clone)]
enum VisitState {
    Visiting,
    Done(Vec<BridgeNodeId>),
}

struct Traversal<'mapper, 'reporter, M>
where
    M: ErrorStackContextMapper + ?Sized,
{
    mapper: &'mapper M,
    builder: BridgeOutputBuilder<'reporter>,
    visited: BTreeMap<usize, VisitState>,
    context_frames: usize,
    attachment_frames: usize,
}

impl<'mapper, 'reporter, M> Traversal<'mapper, 'reporter, M>
where
    M: ErrorStackContextMapper + ?Sized,
{
    fn new(
        mapper: &'mapper M,
        reporter: &'reporter Reporter,
    ) -> Result<Self, ErrorStackBridgeError> {
        Ok(Self {
            mapper,
            builder: BridgeOutputBuilder::new(reporter, PRODUCER)?,
            visited: BTreeMap::new(),
            context_frames: 0,
            attachment_frames: 0,
        })
    }

    /// Returns the nearest context node(s) represented at or below this frame.
    ///
    /// Frame addresses are used only as ephemeral traversal memoization keys.
    /// They never become `BridgeNodeId`, canonical identity, or durable output.
    fn walk(
        &mut self,
        frame: &Frame,
        depth: usize,
    ) -> Result<Vec<BridgeNodeId>, ErrorStackBridgeError> {
        let frame_key = frame as *const Frame as usize;

        if let Some(state) = self.visited.get(&frame_key) {
            return Ok(match state {
                VisitState::Visiting => Vec::new(),
                VisitState::Done(nodes) => nodes.clone(),
            });
        }

        self.visited.insert(frame_key, VisitState::Visiting);

        let nearest = match frame.kind() {
            FrameKind::Context(context) => {
                self.context_frames = self.context_frames.saturating_add(1);

                let metadata = self.mapper.map(ErrorStackContextView {
                    frame,
                    context,
                    depth,
                })?;

                let current = self.builder.push(metadata)?;
                let mut source_nodes = BTreeSet::new();

                for source in frame.sources() {
                    source_nodes.extend(self.walk(source, depth.saturating_add(1))?);
                }

                for source in source_nodes {
                    self.builder.relate(
                        source,
                        current,
                        DiagnosticRelationshipKind::ContributesTo,
                        DiagnosticRelationshipEvidence::SourceChain,
                    )?;
                }

                vec![current]
            }

            FrameKind::Attachment(_) => {
                self.attachment_frames = self.attachment_frames.saturating_add(1);

                let mut nearest = BTreeSet::new();

                for source in frame.sources() {
                    nearest.extend(self.walk(source, depth.saturating_add(1))?);
                }

                nearest.into_iter().collect()
            }
        };

        self.visited
            .insert(frame_key, VisitState::Done(nearest.clone()));

        Ok(nearest)
    }

    fn finish(self) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError> {
        Ok(ErrorStackBridgeOutput {
            bridge: self.builder.finish()?,
            context_frames: self.context_frames,
            attachment_frames: self.attachment_frames,
        })
    }
}

#[derive(Debug)]
pub enum ErrorStackBridgeError {
    Bridge(BridgeError),
}

impl fmt::Display for ErrorStackBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(source) => {
                write!(formatter, "error-stack bridge conversion failed: {source}")
            }
        }
    }
}

impl Error for ErrorStackBridgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bridge(source) => Some(source),
        }
    }
}

impl From<BridgeError> for ErrorStackBridgeError {
    fn from(source: BridgeError) -> Self {
        Self::Bridge(source)
    }
}
