//! Trace 记录与 [`ObjectGraph`](athena_gc::ObjectGraph)。

use athena_gc::{GcObjectId, Trace, Tracer};

use super::ids::{ArrayChunkId, ArrayId, ArrayRevision, ArrayRevisionId, ArraySnapshot, ArraySnapshotId};

/// 测试用 Tracer。
#[derive(Debug, Default, Clone)]
pub struct RecordingTracer {
    /// 标记过的对象。
    pub marked: Vec<GcObjectId>,
}

impl Tracer for RecordingTracer {
    fn mark_object(&mut self, id: GcObjectId) {
        self.marked.push(id);
    }

    fn mark_allocation(&mut self, _payload: *const u8) {}
}

/// Snapshot Trace 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArraySnapshotRecord {
    /// GC 身份。
    pub id: ArraySnapshotId,
    /// Wire 快照。
    pub snapshot: ArraySnapshot,
    /// 对应 revision 对象。
    pub revision_id: ArrayRevisionId,
    /// 元素块。
    pub chunks: Vec<ArrayChunkId>,
}

impl Trace for ArraySnapshotRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        tracer.mark_object(self.revision_id.as_object());
        for chunk in &self.chunks {
            tracer.mark_object(chunk.as_object());
        }
    }
}

/// Revision Trace 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayRevisionRecord {
    /// GC 身份。
    pub id: ArrayRevisionId,
    /// 逻辑数组。
    pub array_id: ArrayId,
    /// 修订号码。
    pub revision: ArrayRevision,
    /// 绑定 snapshot。
    pub snapshot_id: Option<ArraySnapshotId>,
    /// Chunks。
    pub chunks: Vec<ArrayChunkId>,
}

impl Trace for ArrayRevisionRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        if let Some(snapshot_id) = self.snapshot_id {
            tracer.mark_object(snapshot_id.as_object());
        }
        for chunk in &self.chunks {
            tracer.mark_object(chunk.as_object());
        }
    }
}

/// Chunk Trace 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayChunkRecord {
    /// Chunk 身份。
    pub id: ArrayChunkId,
}

impl Trace for ArrayChunkRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
    }
}

/// Collect 用对象出边表。
#[derive(Debug, Default, Clone)]
pub struct ArrayTraceIndex {
    snapshots: std::collections::HashMap<GcObjectId, ArraySnapshotRecord>,
    revisions: std::collections::HashMap<GcObjectId, ArrayRevisionRecord>,
    chunks: std::collections::HashMap<GcObjectId, ArrayChunkRecord>,
}

impl ArrayTraceIndex {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记 snapshot。
    pub fn insert_snapshot(&mut self, record: ArraySnapshotRecord) {
        self.snapshots.insert(record.id.as_object(), record);
    }

    /// 登记 revision。
    pub fn insert_revision(&mut self, record: ArrayRevisionRecord) {
        self.revisions.insert(record.id.as_object(), record);
    }

    /// 登记 chunk。
    pub fn insert_chunk(&mut self, record: ArrayChunkRecord) {
        self.chunks.insert(record.id.as_object(), record);
    }
}

impl athena_gc::ObjectGraph for ArrayTraceIndex {
    fn trace_object(&self, id: GcObjectId, tracer: &mut dyn Tracer) {
        if let Some(record) = self.snapshots.get(&id) {
            record.trace(tracer);
            return;
        }
        if let Some(record) = self.revisions.get(&id) {
            record.trace(tracer);
            return;
        }
        if let Some(record) = self.chunks.get(&id) {
            record.trace(tracer);
        }
    }
}
