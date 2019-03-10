//! 普通离散图结构基座（身份 · 存储 · 视图 · L0 原语）。
//!
//! **不是** M-Graph：不含等价类、witness、hyper-edge、closure 或 solver frontier。
//! CSR / CSC / 工作区复用 [`athena_ndarray`] 的 storage 与 memory budget 合同。
//!
//! 四层目录：[`identity`] · [`storage`] · [`views`] · [`primitives`]。
//! 图论数学结论在 `athena-engine::graph_theory`。

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod identity;
pub mod primitives;
pub mod storage;
pub mod views;

mod error;

pub use error::GraphError;
pub use identity::{
    EdgeId, EdgeRef, Graph, GraphBuilder, GraphDirection, GraphFingerprint, GraphId, GraphRevision, GraphSemantics,
    GraphSnapshot, GraphStorageMetadata, GraphView, ImmutableGraph, MultiplicityPolicy, NodeId, NodeRef, RepresentationId,
    SelfLoopDegree, SourceEdgeRef, SourceNodeRef, ViewEdgeRef, ViewFingerprint, ViewMapping, ViewNodeRef, ViewTransform,
};
pub use primitives::{
    CancelFlag, DeterministicBfsOutcome, DeterministicFrontier, FrontierCheckpoint, UnionFind, bfs_order, deterministic_bfs,
    resume_deterministic_bfs, sort_neighbors_deterministic,
};
pub use storage::{
    CscGraph, CsrGraph, DerivedCsc, GraphAlgorithmRequirements, GraphCapabilities, PropertyCell, PropertyColumn, PropertyStore,
    WeightColumn, WeightDomainTag, csr_to_csc, edge_list_to_csr, graph_edge_list, graph_to_csr,
};
pub use views::{EdgeFilteredView, InducedSubgraphView, ReversedGraphView};
