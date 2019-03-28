//! `finish()` 发布：immutable snapshot + heap 对象根。

use athena_gc::{GcHeap, RootKind, RootToken};

use crate::{ArrayError, ArrayLayout, LogicalShape};

use super::alloc::{allocate_array_chunk_id, allocate_array_revision_id, allocate_array_snapshot_id};
use super::ids::{ArrayChunkId, ArrayId, ArrayRevision, ArrayRevisionId, ArraySnapshot, ArraySnapshotId};
use super::trace_records::{ArrayChunkRecord, ArrayRevisionRecord, ArraySnapshotRecord, ArrayTraceIndex};

/// Heap 发布后的数组身份与 Trace 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayPublication {
    /// 逻辑数组。
    pub array_id: ArrayId,
    /// Wire 修订号。
    pub revision: ArrayRevision,
    /// Revision 对象身份。
    pub revision_id: ArrayRevisionId,
    /// Snapshot 对象身份。
    pub snapshot_id: ArraySnapshotId,
    /// Snapshot root。
    pub snapshot_root: RootToken,
    /// Revision root。
    pub revision_root: RootToken,
    /// Chunk roots。
    pub chunk_roots: Vec<RootToken>,
    /// Chunk 列表。
    pub chunks: Vec<ArrayChunkId>,
    /// Snapshot Trace 记录。
    pub snapshot_record: ArraySnapshotRecord,
    /// Revision Trace 记录。
    pub revision_record: ArrayRevisionRecord,
}

impl ArrayPublication {
    /// 构造 collect 用 [`ArrayTraceIndex`]。
    pub fn trace_index(&self) -> ArrayTraceIndex {
        let mut index = ArrayTraceIndex::new();
        index.insert_snapshot(self.snapshot_record.clone());
        index.insert_revision(self.revision_record.clone());
        for id in &self.chunks {
            index.insert_chunk(ArrayChunkRecord { id: *id });
        }
        index
    }
}

/// 已发布的数组观测（身份层；payload 仍由调用方 storage 持有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedArray {
    /// Wire 快照。
    pub snapshot: ArraySnapshot,
    /// Heap 发布信息。
    pub publication: ArrayPublication,
}

impl PublishedArray {
    /// Snapshot GC 身份。
    pub const fn snapshot_id(&self) -> ArraySnapshotId {
        self.publication.snapshot_id
    }
}

/// 在 heap 上发布数组 snapshot（分配真实 object id 并 root）。
pub fn publish_array_snapshot(
    heap: &mut GcHeap,
    array_id: ArrayId,
    revision: ArrayRevision,
    shape: LogicalShape,
    layout: ArrayLayout,
    chunks: Vec<ArrayChunkId>,
) -> Result<PublishedArray, ArrayError> {
    layout.validate()?;
    if layout.shape != shape {
        return Err(ArrayError::LayoutMismatch);
    }
    let revision_id = allocate_array_revision_id(heap)?;
    let snapshot_id = allocate_array_snapshot_id(heap)?;
    let wire = ArraySnapshot::new(array_id, revision, shape, layout);
    let snapshot_record = ArraySnapshotRecord {
        id: snapshot_id,
        snapshot: wire.clone(),
        revision_id,
        chunks: chunks.clone(),
    };
    let revision_record = ArrayRevisionRecord {
        id: revision_id,
        array_id,
        revision,
        snapshot_id: Some(snapshot_id),
        chunks: chunks.clone(),
    };
    let snapshot_root = heap.roots_mut().register(snapshot_id.as_object(), RootKind::Array);
    let revision_root = heap.roots_mut().register(revision_id.as_object(), RootKind::Array);
    let chunk_roots = chunks
        .iter()
        .map(|id| heap.roots_mut().register(id.as_object(), RootKind::Array))
        .collect();
    Ok(PublishedArray {
        snapshot: wire,
        publication: ArrayPublication {
            array_id,
            revision,
            revision_id,
            snapshot_id,
            snapshot_root,
            revision_root,
            chunk_roots,
            chunks,
            snapshot_record,
            revision_record,
        },
    })
}

/// 便利路径：分配一个 chunk 对象并发布 revision=0 snapshot。
pub fn finish_array_on_heap(
    heap: &mut GcHeap,
    shape: LogicalShape,
    layout: ArrayLayout,
) -> Result<PublishedArray, ArrayError> {
    let array_id = ArrayId::allocate();
    let chunk = allocate_array_chunk_id(heap)?;
    publish_array_snapshot(heap, array_id, ArrayRevision(0), shape, layout, vec![chunk])
}
