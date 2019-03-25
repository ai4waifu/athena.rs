//! Trace 适配记录：Snapshot / Revision / Chunk / View / Workspace。

use athena_gc::{GcObjectId, Trace, Tracer};

use crate::{GraphId, GraphRevision, GraphSnapshot, ViewFingerprint};

use super::{
    chunk::ChunkSet,
    ids::{GraphChunkId, GraphRevisionId, GraphSnapshotId, GraphViewId, GraphWorkspaceId, SpillObjectId},
};

/// 测试 / 诊断用 Tracer：记录 `mark_object` 调用。
#[derive(Debug, Default, Clone)]
pub struct RecordingTracer {
    /// 被标记的对象（含重复）。
    pub marked: Vec<GcObjectId>,
}

impl Tracer for RecordingTracer {
    fn mark_object(&mut self, id: GcObjectId) {
        self.marked.push(id);
    }

    fn mark_allocation(&mut self, _payload: *const u8) {}
}

/// 不可变 revision 记录（Trace 边 → snapshot / chunk set）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRevisionRecord {
    /// GC 身份。
    pub id: GraphRevisionId,
    /// 逻辑图。
    pub graph_id: GraphId,
    /// 修订号码（wire）。
    pub revision: GraphRevision,
    /// 绑定的 snapshot（若已封存）。
    pub snapshot_id: Option<GraphSnapshotId>,
    /// 该版本持有的 chunks。
    pub chunks: ChunkSet,
}

impl Trace for GraphRevisionRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        if let Some(snapshot_id) = self.snapshot_id {
            tracer.mark_object(snapshot_id.as_object());
        }
        self.chunks.trace(tracer);
    }
}

/// 算法可读 snapshot 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSnapshotRecord {
    /// GC 身份。
    pub id: GraphSnapshotId,
    /// Wire 快照字段。
    pub snapshot: GraphSnapshot,
    /// 对应 revision 对象。
    pub revision_id: GraphRevisionId,
    /// 物理 chunks。
    pub chunks: ChunkSet,
    /// 可选挂载视图。
    pub view_id: Option<GraphViewId>,
}

impl Trace for GraphSnapshotRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        tracer.mark_object(self.revision_id.as_object());
        self.chunks.trace(tracer);
        if let Some(view_id) = self.view_id {
            tracer.mark_object(view_id.as_object());
        }
    }
}

/// Chunk 级 Trace 记录（含可选 spill）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphChunkRecord {
    /// Chunk 身份。
    pub id: GraphChunkId,
    /// Spill backing。
    pub spill: Option<SpillObjectId>,
}

impl Trace for GraphChunkRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        if let Some(spill) = self.spill {
            tracer.mark_object(spill.as_object());
        }
    }
}

/// 视图 Trace 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphViewRecord {
    /// 视图身份。
    pub id: GraphViewId,
    /// 源 snapshot。
    pub source_snapshot: GraphSnapshotId,
    /// 视图指纹（wire）。
    pub fingerprint: ViewFingerprint,
}

impl Trace for GraphViewRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        tracer.mark_object(self.source_snapshot.as_object());
    }
}

/// 算法 workspace / checkpoint Trace 记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphWorkspaceRecord {
    /// Workspace 身份。
    pub id: GraphWorkspaceId,
    /// 绑定的 snapshot。
    pub snapshot_id: GraphSnapshotId,
    /// 绑定的 revision 对象。
    pub revision_id: GraphRevisionId,
    /// 需要的 chunk 集合。
    pub chunks: ChunkSet,
}

impl Trace for GraphWorkspaceRecord {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        tracer.mark_object(self.snapshot_id.as_object());
        tracer.mark_object(self.revision_id.as_object());
        self.chunks.trace(tracer);
    }
}

/// 供 [`athena_gc::GcHeap::collect_traced`] 使用的图对象出边表。
#[derive(Debug, Default, Clone)]
pub struct GraphTraceIndex {
    snapshots: std::collections::HashMap<GcObjectId, GraphSnapshotRecord>,
    revisions: std::collections::HashMap<GcObjectId, GraphRevisionRecord>,
    chunks: std::collections::HashMap<GcObjectId, GraphChunkRecord>,
    views: std::collections::HashMap<GcObjectId, GraphViewRecord>,
    workspaces: std::collections::HashMap<GcObjectId, GraphWorkspaceRecord>,
}

impl GraphTraceIndex {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记 snapshot。
    pub fn insert_snapshot(&mut self, record: GraphSnapshotRecord) {
        self.snapshots.insert(record.id.as_object(), record);
    }

    /// 登记 revision。
    pub fn insert_revision(&mut self, record: GraphRevisionRecord) {
        self.revisions.insert(record.id.as_object(), record);
    }

    /// 登记 chunk。
    pub fn insert_chunk(&mut self, record: GraphChunkRecord) {
        self.chunks.insert(record.id.as_object(), record);
    }

    /// 登记 view。
    pub fn insert_view(&mut self, record: GraphViewRecord) {
        self.views.insert(record.id.as_object(), record);
    }

    /// 登记 workspace。
    pub fn insert_workspace(&mut self, record: GraphWorkspaceRecord) {
        self.workspaces.insert(record.id.as_object(), record);
    }
}

impl athena_gc::ObjectGraph for GraphTraceIndex {
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
            return;
        }
        if let Some(record) = self.views.get(&id) {
            record.trace(tracer);
            return;
        }
        if let Some(record) = self.workspaces.get(&id) {
            record.trace(tracer);
        }
    }
}
