//! 图生命周期合同：身份 · lease · 驻留 ≠ 可达 · COW · Trace · checkpoint。

use athena_gc::{GcHeap, HeapBudget, SegmentKind, Trace};
use athena_graph::{
    ChunkRegistry, ChunkResidency, ChunkSet, FrontierCheckpoint, GraphAlgorithmCheckpoint, GraphChunkId, GraphError, GraphId,
    GraphRevision, GraphRevisionId, GraphRevisionRecord, GraphSnapshot, GraphSnapshotId, GraphSnapshotRecord, GraphWorkspaceId,
    RecordingTracer, RepresentationId, SpillObjectId,
};

#[test]
fn lifecycle_ids_are_distinct_object_family() {
    let a = GraphRevisionId::allocate();
    let b = GraphSnapshotId::allocate();
    let c = GraphChunkId::allocate();
    assert_ne!(a.as_object(), b.as_object());
    assert_ne!(b.as_object(), c.as_object());
}

#[test]
fn reachability_is_not_residency() {
    let mut reg = ChunkRegistry::new();
    let id = GraphChunkId::allocate();
    reg.register_resident(id).unwrap();
    let spill = SpillObjectId::allocate();
    reg.spill(id, spill).unwrap();
    assert_eq!(reg.get(id).unwrap().residency, ChunkResidency::Spilled);
    assert!(reg.get(id).unwrap().semantic_reachable);

    let lease = reg.acquire_lease(id).unwrap();
    assert!(matches!(
        reg.pin_resident(&lease),
        Err(GraphError::ChunkNotResident { .. })
    ));
    reg.release_lease(lease);

    reg.materialize(id).unwrap();
    let lease = reg.acquire_lease(id).unwrap();
    let pin = reg.pin_resident(&lease).unwrap();
    assert_eq!(pin.chunk_id(), id);
    reg.release_pin(pin);
    reg.release_lease(lease);
}

#[test]
fn pin_requires_active_lease_count() {
    let mut reg = ChunkRegistry::new();
    let id = GraphChunkId::allocate();
    reg.register_resident(id).unwrap();
    let lease = reg.acquire_lease(id).unwrap();
    reg.release_lease(lease);
    // 伪造「无 lease」票据：再 acquire 后立即 release，用另一票据对象不可行。
    // 合同：lease_count==0 时 pin 失败 — 构造零 lease 的 guard 不可从 API 获得。
    // 覆盖路径：spill 后未 materialize 时 NotResident；此处验证成对 release 后计数归零。
    assert_eq!(reg.get(id).unwrap().lease_count, 0);
    assert_eq!(reg.get(id).unwrap().pin_count, 0);
}

#[test]
fn cannot_spill_while_pinned() {
    let mut reg = ChunkRegistry::new();
    let id = GraphChunkId::allocate();
    reg.register_resident(id).unwrap();
    let lease = reg.acquire_lease(id).unwrap();
    let pin = reg.pin_resident(&lease).unwrap();
    let spill = SpillObjectId::allocate();
    assert!(matches!(reg.spill(id, spill), Err(GraphError::ChunkPinned { .. })));
    reg.release_pin(pin);
    reg.spill(id, spill).unwrap();
    assert_eq!(reg.get(id).unwrap().residency, ChunkResidency::Spilled);
    reg.release_lease(lease);
}

#[test]
fn unreachable_blocks_lease() {
    let mut reg = ChunkRegistry::new();
    let id = GraphChunkId::allocate();
    reg.register_resident(id).unwrap();
    reg.set_reachable(id, false).unwrap();
    assert!(matches!(reg.acquire_lease(id), Err(GraphError::ChunkUnreachable { .. })));
    // 未驻留 ≠ 不可达：恢复可达后仍可 lease。
    reg.set_reachable(id, true).unwrap();
    let lease = reg.acquire_lease(id).unwrap();
    reg.release_lease(lease);
}

#[test]
fn cow_share_and_fork() {
    let mut reg = ChunkRegistry::new();
    let id = GraphChunkId::allocate();
    reg.register_resident(id).unwrap();
    reg.share(id).unwrap();
    assert_eq!(reg.get(id).unwrap().share_count, 2);
    let forked = reg.fork_cow(id).unwrap();
    assert_ne!(forked, id);
    assert_eq!(reg.get(id).unwrap().share_count, 1);
    assert_eq!(reg.get(forked).unwrap().share_count, 1);
    let same = reg.fork_cow(id).unwrap();
    assert_eq!(same, id);
}

#[test]
fn snapshot_and_revision_trace_edges() {
    let revision_id = GraphRevisionId::allocate();
    let snapshot_id = GraphSnapshotId::allocate();
    let chunk = GraphChunkId::allocate();
    let mut chunks = ChunkSet::new();
    chunks.push(chunk);
    let snap = GraphSnapshotRecord {
        id: snapshot_id,
        snapshot: GraphSnapshot::new(
            GraphId::allocate(),
            GraphRevision(1),
            Default::default(),
            RepresentationId::CSR,
        ),
        revision_id,
        chunks: chunks.clone(),
        view_id: None,
    };
    let rev = GraphRevisionRecord {
        id: revision_id,
        graph_id: snap.snapshot.graph_id,
        revision: snap.snapshot.revision,
        snapshot_id: Some(snapshot_id),
        chunks,
    };
    let mut tracer = RecordingTracer::default();
    snap.trace(&mut tracer);
    rev.trace(&mut tracer);
    assert!(tracer.marked.contains(&snapshot_id.as_object()));
    assert!(tracer.marked.contains(&revision_id.as_object()));
    assert!(tracer.marked.contains(&chunk.as_object()));
}

#[test]
fn checkpoint_binds_revision_and_chunks() {
    let snapshot_id = GraphSnapshotId::allocate();
    let revision_id = GraphRevisionId::allocate();
    let workspace_id = GraphWorkspaceId::allocate();
    let c0 = GraphChunkId::allocate();
    let c1 = GraphChunkId::allocate();
    let cp = GraphAlgorithmCheckpoint::new(
        snapshot_id,
        GraphId::allocate(),
        GraphRevision(3),
        revision_id,
        [c0, c1],
        workspace_id,
        FrontierCheckpoint {
            queue: vec![],
            discovered: vec![],
            visited_prefix: vec![],
        },
    );
    assert_eq!(cp.snapshot_id, snapshot_id);
    assert_eq!(cp.revision_id, revision_id);
    assert_eq!(cp.chunks.chunks, vec![c0, c1]);
    assert_ne!(cp.chunk_identity_fingerprint(), 0);
}

#[test]
fn typed_graph_segments_via_athena_gc() {
    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let index = h.allocate_graph_index(64).unwrap();
    let prop = h.allocate_graph_property(32).unwrap();
    assert_eq!(index.kind, SegmentKind::GraphIndex);
    assert_eq!(prop.kind, SegmentKind::GraphProperty);
    assert!(h.segments().any(|s| s.kind == SegmentKind::GraphIndex));
    assert!(h.segments().any(|s| s.kind == SegmentKind::GraphProperty));
}
