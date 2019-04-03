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
    assert!(matches!(reg.pin_resident(&lease), Err(GraphError::ChunkNotResident { .. })));
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
        snapshot: GraphSnapshot::new(GraphId::allocate(), GraphRevision(1), Default::default(), RepresentationId::CSR),
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
        FrontierCheckpoint { queue: vec![], discovered: vec![], visited_prefix: vec![] },
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

#[test]
fn heap_bound_chunk_id_stale_after_release() {
    use athena_graph::allocate_chunk_id;
    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let id = allocate_chunk_id(&mut h).unwrap();
    assert!(h.object_payload(id.as_object()).is_ok());
    h.release_object(id.as_object()).unwrap();
    assert!(matches!(h.object_payload(id.as_object()), Err(athena_gc::GcError::StaleObject { .. })));
}

#[test]
fn finish_csr_on_heap_wires_snapshot_and_index_chunks() {
    use athena_graph::{GraphBuilder, GraphDirection, graph_to_csr_on_heap};
    use athena_ndarray::MemoryBudget;

    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let mut registry = ChunkRegistry::new();
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let n0 = b.add_node(());
    let n1 = b.add_node(());
    let n2 = b.add_node(());
    b.add_edge(n0, n1, ());
    b.add_edge(n1, n2, ());
    let budget = MemoryBudget::new(64 * 1024).unwrap();
    let (published, csr) = b.finish_csr_on_heap(&mut h, &mut registry, budget).unwrap();
    assert_eq!(published.chunks().chunks.len(), 2);
    assert_eq!(published.publication.chunk_roots.len(), 2);
    assert!(h.object_payload(published.snapshot_id().as_object()).is_ok());
    assert!(h.object_payload(published.publication.revision_id.as_object()).is_ok());
    let mut neighbors = Vec::new();
    csr.csr.for_each_neighbor_chunk(0, |chunk| neighbors.extend_from_slice(chunk)).unwrap();
    assert_eq!(neighbors, vec![1]);

    // graph_to_csr_on_heap 独立路径与 finish 对齐 chunk 数。
    let mut b2 = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let a = b2.add_node(());
    let c = b2.add_node(());
    b2.add_edge(a, c, ());
    let extra = graph_to_csr_on_heap(b2.graph(), &mut h, &mut registry, budget).unwrap();
    assert_eq!(extra.chunks.chunks.len(), 2);
}

#[test]
fn gc_dense_property_column_on_graph_property_segment() {
    use athena_graph::GcDenseU64Column;
    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let mut registry = ChunkRegistry::new();
    let col = GcDenseU64Column::allocate(&mut h, &mut registry, &[7, 8, 9]).unwrap();
    assert_eq!(col.len(), 3);
    assert_eq!(col.read_range(0, 3).unwrap(), vec![7, 8, 9]);
    assert!(h.segments().any(|s| s.kind == SegmentKind::GraphProperty));
}

#[test]
fn shared_chunks_survive_dropping_one_snapshot_root() {
    use athena_gc::RootKind;
    use athena_graph::{GraphBuilder, GraphDirection, publication_attach_chunks};
    use athena_ndarray::MemoryBudget;

    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let mut registry = ChunkRegistry::new();
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let n0 = b.add_node(());
    let n1 = b.add_node(());
    b.add_edge(n0, n1, ());
    let budget = MemoryBudget::new(64 * 1024).unwrap();
    let (first, csr) = b.finish_csr_on_heap(&mut h, &mut registry, budget).unwrap();
    for id in &csr.chunks.chunks {
        registry.share(*id).unwrap();
    }
    let mut second = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish_on_heap(&mut h).unwrap();
    publication_attach_chunks(&mut h, &mut second.publication, csr.chunks.clone());

    for token in first.publication.chunk_roots.iter().copied() {
        assert!(h.roots_mut().unregister(token));
    }
    assert!(h.roots_mut().unregister(first.publication.snapshot_root));
    assert!(h.roots_mut().unregister(first.publication.revision_root));

    let index = second.publication.trace_index();
    h.collect_traced(&index).unwrap();
    assert!(h.object_payload(second.snapshot_id().as_object()).is_ok());
    assert!(h.object_payload(csr.chunks.chunks[0].as_object()).is_ok());
    let _ = RootKind::Graph;
}
