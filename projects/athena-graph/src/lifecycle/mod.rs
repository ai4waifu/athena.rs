//! 图生命周期：身份、lease、驻留状态机、ChunkSet、Trace 与 checkpoint 形状。
//!
//! 与 `athena-gc` 共用 runtime heap。本模块提供图侧合同与 Trace 适配。
//! spill / LRU / 调度策略在 `athena-engine`。

mod checkpoint;
mod chunk;
mod ids;
mod lease;
mod residency;
mod trace_records;

pub use checkpoint::GraphAlgorithmCheckpoint;
pub use chunk::{ChunkMeta, ChunkSet};
pub use ids::{
    GraphChunkId, GraphRevisionId, GraphSnapshotId, GraphViewId, GraphWorkspaceId, SpillObjectId, allocate_lifecycle_object_id,
};
pub use lease::{ChunkLeaseGuard, ChunkRegistry, GcRootToken, ResidentPinGuard};
pub use residency::ChunkResidency;
pub use trace_records::{
    GraphChunkRecord, GraphRevisionRecord, GraphSnapshotRecord, GraphViewRecord, GraphWorkspaceRecord, RecordingTracer,
};
