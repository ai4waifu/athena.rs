//! 图生命周期：身份、lease、驻留状态机、ChunkSet、Trace 与 checkpoint 形状。
//!
//! 与 `athena-gc` 共用 runtime heap。本模块提供图侧合同与 Trace 适配。
//! spill / LRU / 调度策略在 `athena-engine`。

mod alloc;
mod checkpoint;
mod chunk;
mod ids;
mod lease;
mod publish;
mod residency;
mod trace_records;

pub use alloc::{
    allocate_chunk_id, allocate_revision_id, allocate_snapshot_id, allocate_spill_id, allocate_view_id, allocate_workspace_id,
};
pub use checkpoint::GraphAlgorithmCheckpoint;
pub use chunk::{ChunkMeta, ChunkSet};
pub use ids::{
    GraphChunkId, GraphRevisionId, GraphSnapshotId, GraphViewId, GraphWorkspaceId, SpillObjectId, allocate_lifecycle_object_id,
};
pub use lease::{ChunkLeaseGuard, ChunkRegistry, GcRootToken, ResidentPinGuard};
pub use publish::{
    finish_on_heap, publication_attach_chunks, publish_immutable_graph, GraphPublication, PublishedImmutableGraph,
};
pub use residency::ChunkResidency;
pub use trace_records::{
    GraphChunkRecord, GraphRevisionRecord, GraphSnapshotRecord, GraphTraceIndex, GraphViewRecord, GraphWorkspaceRecord,
    RecordingTracer,
};
