//! 图身份层：逻辑图、修订、快照与结构语义。

mod direction;
mod graph;
mod id;
mod semantics;

pub use direction::GraphDirection;
pub use graph::{MutableGraph, GraphBuilder, GraphView, ImmutableGraph};
pub use id::{
    EdgeId, EdgeRef, GraphId, GraphRevision, NodeId, NodeRef, RepresentationId, SourceEdgeRef, SourceNodeRef, ViewEdgeRef,
    ViewNodeRef,
};
pub use semantics::{
    GraphFingerprint, GraphSemantics, GraphSnapshot, GraphStorageMetadata, MultiplicityPolicy, SelfLoopDegree, ViewFingerprint,
    ViewMapping, ViewTransform,
};
