//! 在真实 `GcHeap` 上分配 Array*Id。

use athena_gc::{GcError, GcHeap};

use super::ids::{ArrayChunkId, ArrayRevisionId, ArraySnapshotId};

fn alloc_object_id(heap: &mut GcHeap) -> Result<athena_gc::GcObjectId, GcError> {
    heap.allocate_object(8)
}

/// 分配真实 heap 绑定的 [`ArrayRevisionId`]。
pub fn allocate_array_revision_id(heap: &mut GcHeap) -> Result<ArrayRevisionId, GcError> {
    Ok(ArrayRevisionId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`ArraySnapshotId`]。
pub fn allocate_array_snapshot_id(heap: &mut GcHeap) -> Result<ArraySnapshotId, GcError> {
    Ok(ArraySnapshotId::from_object(alloc_object_id(heap)?))
}

/// 分配真实 heap 绑定的 [`ArrayChunkId`]。
pub fn allocate_array_chunk_id(heap: &mut GcHeap) -> Result<ArrayChunkId, GcError> {
    Ok(ArrayChunkId::from_object(alloc_object_id(heap)?))
}
