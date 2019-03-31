//! 数组生命周期：身份、heap 发布与 Trace（简于图）。

mod alloc;
mod ids;
mod publish;
mod trace_records;

pub use alloc::{allocate_array_chunk_id, allocate_array_revision_id, allocate_array_snapshot_id};
pub use ids::{ArrayChunkId, ArrayId, ArrayRevision, ArrayRevisionId, ArraySnapshot, ArraySnapshotId};
pub use publish::{ArrayPublication, PublishedArray, finish_array_on_heap, publish_array_snapshot};
pub use trace_records::{ArrayChunkRecord, ArrayRevisionRecord, ArraySnapshotRecord, ArrayTraceIndex, RecordingTracer};
