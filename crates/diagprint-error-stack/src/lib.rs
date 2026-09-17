//! Structured [`error_stack`] interoperability for diagprint.
//!
//! This crate is the first ecosystem adapter built on
//! [`diagprint_bridge`]. It keeps `error-stack`-specific traversal and privacy
//! policy here while delegating normalized diagnostic/report/relationship
//! assembly to the reusable bridge SDK.
//!
//! Stable single-context [`error_stack::Report<C>`] and grouped
//! `Report<[C]>` values are both supported.
//!
//! # Structural conversion
//!
//! The adapter uses [`error_stack::Frame::sources`] and
//! [`error_stack::Frame::kind`] directly. It never parses the rendered
//! `Display` or `Debug` representation of a report.
//!
//! Context frames become diagnostic instances. Attachment frames are
//! traversal-transparent so source topology survives across attachments.
//!
//! # Attachment privacy
//!
//! Attachment content is omitted by default. Call
//! [`ErrorStackBridge::with_attachment_policy`] with
//! [`ErrorStackAttachmentPolicy::PrintableText`] to intentionally retain the
//! `Display` text of printable attachments as diagnostic notes.
//!
//! Opaque attachment values are never exported by this adapter.

use diagprint::{
    DiagnosticRelationshipEvidence, DiagnosticRelationshipGraph, DiagnosticRelationshipKind,
    DiagnosticReport, Reporter,
};
use diagprint_bridge::{
    BridgeBuildStats, BridgeDiagnosticMetadata, BridgeError, BridgeNodeId, BridgeOutput,
    BridgeOutputBuilder,
};
use error_stack::{AttachmentKind, Frame, FrameKind, Report};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

const PRODUCER: &str = "error-stack";
const ATTACHMENT_NOTE_PREFIX: &str = "error-stack attachment: ";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ErrorStackAttachmentPolicy {
    #[default]
    Omit,
    PrintableText,
}

impl ErrorStackAttachmentPolicy {
    pub const fn includes_printable_text(self) -> bool {
        matches!(self, Self::PrintableText)
    }
}

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

#[derive(Debug)]
pub struct ErrorStackBridgeOutput {
    bridge: BridgeOutput,
    context_frames: usize,
    attachment_frames: usize,
    printable_attachment_frames: usize,
    opaque_attachment_frames: usize,
    included_printable_attachments: usize,
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

    pub const fn printable_attachment_frames(&self) -> usize {
        self.printable_attachment_frames
    }

    pub const fn opaque_attachment_frames(&self) -> usize {
        self.opaque_attachment_frames
    }

    pub const fn included_printable_attachments(&self) -> usize {
        self.included_printable_attachments
    }

    pub fn into_bridge(self) -> BridgeOutput {
        self.bridge
    }
}

#[derive(Debug, Clone)]
pub struct ErrorStackBridge<M = DefaultErrorStackContextMapper> {
    mapper: M,
    attachment_policy: ErrorStackAttachmentPolicy,
}

impl ErrorStackBridge<DefaultErrorStackContextMapper> {
    pub const fn new() -> Self {
        Self {
            mapper: DefaultErrorStackContextMapper,
            attachment_policy: ErrorStackAttachmentPolicy::Omit,
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
        Self {
            mapper,
            attachment_policy: ErrorStackAttachmentPolicy::Omit,
        }
    }

    pub const fn with_attachment_policy(
        mut self,
        attachment_policy: ErrorStackAttachmentPolicy,
    ) -> Self {
        self.attachment_policy = attachment_policy;
        self
    }

    pub const fn attachment_policy(&self) -> ErrorStackAttachmentPolicy {
        self.attachment_policy
    }

    pub fn convert<C>(
        &self,
        report: &Report<C>,
        reporter: &Reporter,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        C: Error + Send + Sync + 'static,
    {
        let mut traversal = Traversal::new(&self.mapper, reporter, self.attachment_policy)?;

        traversal.walk(report.current_frame(), 0, Vec::new())?;
        traversal.finish()
    }

    pub fn convert_grouped<C>(
        &self,
        report: &Report<[C]>,
        reporter: &Reporter,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        C: Error + Send + Sync + 'static,
    {
        let mut traversal = Traversal::new(&self.mapper, reporter, self.attachment_policy)?;

        for current in report.current_frames() {
            traversal.walk(current, 0, Vec::new())?;
        }

        traversal.finish()
    }
}

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

    fn to_diagprint_with_policy(
        &self,
        reporter: &Reporter,
        policy: ErrorStackAttachmentPolicy,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>;

    fn to_diagprint_with_mapper_and_policy<M>(
        &self,
        reporter: &Reporter,
        mapper: &M,
        policy: ErrorStackAttachmentPolicy,
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

    fn to_diagprint_with_policy(
        &self,
        reporter: &Reporter,
        policy: ErrorStackAttachmentPolicy,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError> {
        ErrorStackBridge::new()
            .with_attachment_policy(policy)
            .convert(self, reporter)
    }

    fn to_diagprint_with_mapper_and_policy<M>(
        &self,
        reporter: &Reporter,
        mapper: &M,
        policy: ErrorStackAttachmentPolicy,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        M: ErrorStackContextMapper + ?Sized,
    {
        ErrorStackBridge::with_mapper(mapper)
            .with_attachment_policy(policy)
            .convert(self, reporter)
    }
}

impl<C> ErrorStackReportExt for Report<[C]>
where
    C: Error + Send + Sync + 'static,
{
    fn to_diagprint(
        &self,
        reporter: &Reporter,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError> {
        ErrorStackBridge::new().convert_grouped(self, reporter)
    }

    fn to_diagprint_with<M>(
        &self,
        reporter: &Reporter,
        mapper: &M,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        M: ErrorStackContextMapper + ?Sized,
    {
        ErrorStackBridge::with_mapper(mapper).convert_grouped(self, reporter)
    }

    fn to_diagprint_with_policy(
        &self,
        reporter: &Reporter,
        policy: ErrorStackAttachmentPolicy,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError> {
        ErrorStackBridge::new()
            .with_attachment_policy(policy)
            .convert_grouped(self, reporter)
    }

    fn to_diagprint_with_mapper_and_policy<M>(
        &self,
        reporter: &Reporter,
        mapper: &M,
        policy: ErrorStackAttachmentPolicy,
    ) -> Result<ErrorStackBridgeOutput, ErrorStackBridgeError>
    where
        M: ErrorStackContextMapper + ?Sized,
    {
        ErrorStackBridge::with_mapper(mapper)
            .with_attachment_policy(policy)
            .convert_grouped(self, reporter)
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
    attachment_policy: ErrorStackAttachmentPolicy,
    builder: BridgeOutputBuilder<'reporter>,
    visited: BTreeMap<usize, VisitState>,
    context_frames: usize,
    attachment_frames: usize,
    printable_attachment_frames: usize,
    opaque_attachment_frames: usize,
    included_printable_attachments: usize,
}

impl<'mapper, 'reporter, M> Traversal<'mapper, 'reporter, M>
where
    M: ErrorStackContextMapper + ?Sized,
{
    fn new(
        mapper: &'mapper M,
        reporter: &'reporter Reporter,
        attachment_policy: ErrorStackAttachmentPolicy,
    ) -> Result<Self, ErrorStackBridgeError> {
        Ok(Self {
            mapper,
            attachment_policy,
            builder: BridgeOutputBuilder::new(reporter, PRODUCER)?,
            visited: BTreeMap::new(),
            context_frames: 0,
            attachment_frames: 0,
            printable_attachment_frames: 0,
            opaque_attachment_frames: 0,
            included_printable_attachments: 0,
        })
    }

    fn walk(
        &mut self,
        frame: &Frame,
        depth: usize,
        pending_printable: Vec<String>,
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

                let mut metadata = self.mapper.map(ErrorStackContextView {
                    frame,
                    context,
                    depth,
                })?;

                for attachment in pending_printable {
                    metadata = metadata.note(format!("{ATTACHMENT_NOTE_PREFIX}{attachment}"));
                    self.included_printable_attachments =
                        self.included_printable_attachments.saturating_add(1);
                }

                let current = self.builder.push(metadata)?;
                let mut source_nodes = BTreeSet::new();

                for source in frame.sources() {
                    source_nodes.extend(self.walk(source, depth.saturating_add(1), Vec::new())?);
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

            FrameKind::Attachment(kind) => {
                self.attachment_frames = self.attachment_frames.saturating_add(1);

                let mut pending_printable = pending_printable;

                match kind {
                    AttachmentKind::Printable(value) => {
                        self.printable_attachment_frames =
                            self.printable_attachment_frames.saturating_add(1);

                        if self.attachment_policy.includes_printable_text() {
                            pending_printable.push(value.to_string());
                        }
                    }

                    AttachmentKind::Opaque(_) => {
                        self.opaque_attachment_frames =
                            self.opaque_attachment_frames.saturating_add(1);
                    }

                    _ => {}
                }

                let mut nearest = BTreeSet::new();

                for source in frame.sources() {
                    nearest.extend(self.walk(
                        source,
                        depth.saturating_add(1),
                        pending_printable.clone(),
                    )?);
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
            printable_attachment_frames: self.printable_attachment_frames,
            opaque_attachment_frames: self.opaque_attachment_frames,
            included_printable_attachments: self.included_printable_attachments,
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
