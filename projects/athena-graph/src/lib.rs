//! 普通离散图结构基座（身份 · 存储 · 视图 · L0 原语）。
//!
//! **不是** M-Graph：不含等价类、witness、hyper-edge、closure 或 solver frontier。
//! CSR / CSC / 工作区复用 [`athena_ndarray`] 的 storage 与 memory budget 合同。
//! 生命周期与 `athena-gc` 共用 runtime heap（见 [`lifecycle`]）。
//!
//! 分层：[`identity`] · [`storage`] · [`views`] · [`primitives`] · [`lifecycle`]。
//! 图论数学结论在 `athena-engine::graph_theory`。

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod identity;
pub mod lifecycle;
pub mod primitives;
pub mod storage;
pub mod views;

mod error;

pub use error::GraphError;
pub use identity::{
    EdgeId, EdgeRef, GraphBuilder, GraphDirection, GraphFingerprint, GraphId, GraphRevision, GraphSemantics, GraphSnapshot,
    GraphStorageMetadata, GraphView, ImmutableGraph, MultiplicityPolicy, MutableGraph, NodeId, NodeRef, RepresentationId, SelfLoopDegree,
    SourceEdgeRef, SourceNodeRef, ViewEdgeRef, ViewFingerprint, ViewMapping, ViewNodeRef, ViewTransform,
};
pub use lifecycle::{
    ChunkLeaseGuard, ChunkMeta, ChunkRegistry, ChunkResidency, ChunkSet, GcRootToken, GraphAlgorithmCheckpoint, GraphChunkId, GraphChunkRecord,
    GraphPublication, GraphRevisionId, GraphRevisionRecord, GraphSnapshotId, GraphSnapshotRecord, GraphTraceIndex, GraphViewId,
    GraphViewRecord, GraphWorkspaceId, GraphWorkspaceRecord, PublishedImmutableGraph, RecordingTracer, ResidentPinGuard, SpillObjectId,
    allocate_chunk_id, allocate_revision_id, allocate_snapshot_id, allocate_spill_id, allocate_view_id, allocate_workspace_id, finish_on_heap,
    publication_attach_chunks, publish_immutable_graph,
};
pub use primitives::{
    CancelFlag, DeterministicBfsOutcome, DeterministicFrontier, FrontierCheckpoint, UnionFind, bfs_order, deterministic_bfs,
    resume_deterministic_bfs, sort_neighbors_deterministic,
};
pub use storage::{
    CscGraph, CsrGraph, CsrOnHeap, DerivedCsc, GcDenseU64Column, GcPayloadStorage, GraphAlgorithmRequirements, GraphCapabilities, PropertyCell,
    PropertyColumn, PropertyStore, WeightColumn, WeightDomainTag, attach_csr_chunks, csr_to_csc, edge_list_to_csr, finish_csr_on_heap,
    graph_edge_list, graph_to_csr, graph_to_csr_on_heap,
};
pub use views::{EdgeFilteredView, InducedSubgraphView, ReversedGraphView};
