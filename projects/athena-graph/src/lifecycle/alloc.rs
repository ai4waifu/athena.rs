//! 在真实 `GcHeap` 上分配 Graph*Id（非引导 atomic）。

use athena_gc::{GcError, GcHeap, Result as GcResult};

use super::ids::{
    GraphChunkId, GraphRevisionId, GraphSnapshotId, GraphViewId, GraphWorkspaceId, SpillObjectId,
};

fn alloc_object_id(heap: &mut GcHeap) -> GcResult<athena_gc::GcObjectId> {
    // 8 字节占位 payload：身份槽，领域元数据在 Trace 记录侧。
    heap.allocate_object(8)
}

/// 分配真实 heap 绑定的 [`GraphRevisionId`]。
pub fn allocate_revision_id(heap: &mut GcHeap) -> Result<GraphRevisionId, GcError> {
    Ok(GraphRevisionId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`GraphSnapshotId`]。
pub fn allocate_snapshot_id(heap: &mut GcHeap) -> Result<GraphSnapshotId, GcError> {
    Ok(GraphSnapshotId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`GraphChunkId`]。
pub fn allocate_chunk_id(heap: &mut GcHeap) -> Result<GraphChunkId, GcError> {
    Ok(GraphChunkId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`GraphViewId`]。
pub fn allocate_view_id(heap: &mut GcHeap) -> Result<GraphViewId, GcError> {
    Ok(GraphViewId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`GraphWorkspaceId`]。
pub fn allocate_workspace_id(heap: &mut GcHeap) -> Result<GraphWorkspaceId, GcError> {
    Ok(GraphWorkspaceId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`SpillObjectId`]。
pub fn allocate_spill_id(heap: &mut GcHeap) -> Result<SpillObjectId, GcError> {
    Ok(SpillObjectId::from_object(alloc_object_id(heap)?))
}
