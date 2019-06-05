//! `finish()` 发布：immutable snapshot + ChunkSet + heap 对象根。

use athena_gc::{GcHeap, RootKind, RootToken};

use crate::{GraphError, GraphId, GraphRevision, GraphSnapshot, ImmutableGraph, MutableGraph, RepresentationId};

use super::{
    alloc::{allocate_revision_id, allocate_snapshot_id},
    chunk::ChunkSet,
    ids::{GraphRevisionId, GraphSnapshotId},
    trace_records::{GraphChunkRecord, GraphRevisionRecord, GraphSnapshotRecord, GraphTraceIndex},
};

/// Session / heap 发布后的图身份与 Trace 记录（不含邻接 payload 本身）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphPublication {
    /// 逻辑图。
    pub graph_id: GraphId,
    /// Wire 修订号。
    pub revision: GraphRevision,
    /// Revision 对象身份。
    pub revision_id: GraphRevisionId,
    /// Snapshot 对象身份。
    pub snapshot_id: GraphSnapshotId,
    /// Snapshot object root。
    pub snapshot_root: RootToken,
    /// Revision object root。
    pub revision_root: RootToken,
    /// Chunk object roots（与 [`Self::chunks`] 对齐）。
    pub chunk_roots: Vec<RootToken>,
    /// 物理 chunk 集合（CSR / 属性等随后登记）。
    pub chunks: ChunkSet,
    /// Trace 记录。
    pub snapshot_record: GraphSnapshotRecord,
    /// Revision Trace 记录。
    pub revision_record: GraphRevisionRecord,
}

impl GraphPublication {
    /// 构造 collect 用 [`GraphTraceIndex`]。
    pub fn trace_index(&self) -> GraphTraceIndex {
        let mut index = GraphTraceIndex::new();
        index.insert_snapshot(self.snapshot_record.clone());
        index.insert_revision(self.revision_record.clone());
        for id in &self.chunks.chunks {
            index.insert_chunk(GraphChunkRecord { id: *id, spill: None });
        }
        index
    }
}

/// 不可变图 + 已登记的 lifecycle 发布信息。
#[derive(Debug)]
pub struct PublishedImmutableGraph<N, E> {
    /// 只读图。
    pub graph: ImmutableGraph<N, E>,
    /// Heap 绑定的身份 / ChunkSet / Trace。
    pub publication: GraphPublication,
}

impl<N, E> PublishedImmutableGraph<N, E> {
    /// Wire 快照。
    pub fn snapshot(&self) -> GraphSnapshot {
        self.graph.snapshot()
    }

    /// Snapshot GC 身份。
    pub const fn snapshot_id(&self) -> GraphSnapshotId {
        self.publication.snapshot_id
    }

    /// ChunkSet。
    pub fn chunks(&self) -> &ChunkSet {
        &self.publication.chunks
    }
}

/// 在 heap 上封存 immutable snapshot（分配真实 object id 并 root）。
pub fn publish_immutable_graph<N, E>(
    graph: ImmutableGraph<N, E>,
    heap: &mut GcHeap,
    representation: RepresentationId,
) -> Result<PublishedImmutableGraph<N, E>, GraphError> {
    let revision_id = allocate_revision_id(heap)?;
    let snapshot_id = allocate_snapshot_id(heap)?;
    let wire = GraphSnapshot::new(graph.id(), graph.revision(), graph.semantics(), representation);
    let chunks = ChunkSet::new();
    let snapshot_record = GraphSnapshotRecord { id: snapshot_id, snapshot: wire, revision_id, chunks: chunks.clone(), view_id: None };
    let revision_record = GraphRevisionRecord {
        id: revision_id,
        graph_id: graph.id(),
        revision: graph.revision(),
        snapshot_id: Some(snapshot_id),
        chunks: chunks.clone(),
    };
    let snapshot_root = heap.roots_mut().register(snapshot_id.as_object(), RootKind::Graph);
    let revision_root = heap.roots_mut().register(revision_id.as_object(), RootKind::Graph);
    Ok(PublishedImmutableGraph {
        graph,
        publication: GraphPublication {
            graph_id: wire.graph_id,
            revision: wire.revision,
            revision_id,
            snapshot_id,
            snapshot_root,
            revision_root,
            chunk_roots: Vec::new(),
            chunks,
            snapshot_record,
            revision_record,
        },
    })
}

/// 将 chunk 并入已发布记录，并为每个 chunk object 登记 [`RootKind::Graph`]。
///
/// COW 共享由调用方 [`super::ChunkRegistry`] 记账。
pub fn publication_attach_chunks(heap: &mut GcHeap, publication: &mut GraphPublication, chunks: ChunkSet) {
    for token in publication.chunk_roots.drain(..) {
        let _ = heap.roots_mut().unregister(token);
    }
    publication.chunks = chunks.clone();
    publication.snapshot_record.chunks = chunks.clone();
    publication.revision_record.chunks = chunks.clone();
    publication.chunk_roots = chunks.chunks.iter().map(|id| heap.roots_mut().register(id.as_object(), RootKind::Graph)).collect();
}

/// Builder 完成并在 heap 上发布（邻接表示，空 ChunkSet）。
pub fn finish_on_heap<N, E>(graph: MutableGraph<N, E>, heap: &mut GcHeap) -> Result<PublishedImmutableGraph<N, E>, GraphError> {
    let immutable = ImmutableGraph::from_mutable(graph);
    publish_immutable_graph(immutable, heap, RepresentationId::ADJACENCY_LIST)
}
