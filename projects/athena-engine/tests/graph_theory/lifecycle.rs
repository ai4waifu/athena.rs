//! Engine 图驻留策略：spill / LRU / checkpoint 接线。

use athena_engine::{GraphResidencyController, Session, bind_algorithm_checkpoint, resume_from_algorithm_checkpoint};
use athena_gc::GcHeap;
use athena_graph::{
    CancelFlag, ChunkRegistry, ChunkResidency, DeterministicBfsOutcome, FrontierCheckpoint, GraphBuilder, GraphDirection,
    GraphError, GraphRevision, NodeId,
};
use athena_ndarray::MemoryBudget;

fn tiny_directed() -> GraphBuilder<(), ()> {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let n0 = b.add_node(());
    let n1 = b.add_node(());
    let n2 = b.add_node(());
    b.add_edge(n0, n1, ());
    b.add_edge(n1, n2, ());
    b
}

#[test]
fn session_finish_graph_registers_object_roots() {
    let session = Session::new();
    let published = session.finish_graph_on_heap(tiny_directed()).unwrap();
    let index = published.publication.trace_index();
    session.collect_traced(&index).unwrap();
    assert!(session.heap().borrow().object_payload(published.snapshot_id().as_object()).is_ok());
}

#[test]
fn lru_spills_unpinned_resident_chunks() {
    let heap = GcHeap::new(athena_gc::HeapBudget::default());
    let mut h = heap.borrow_mut();
    let mut registry = ChunkRegistry::new();
    let budget = MemoryBudget::new(64 * 1024).unwrap();
    let (_published, csr) = tiny_directed().finish_csr_on_heap(&mut h, &mut registry, budget).unwrap();
    let mut ctl = GraphResidencyController::new(1);
    for id in &csr.chunks.chunks {
        ctl.touch(*id);
    }
    let spilled = ctl.enforce_resident_limit(&mut h, &mut registry).unwrap();
    assert!(!spilled.is_empty());
    let spilled_id = spilled[0].0;
    assert_eq!(registry.get(spilled_id).unwrap().residency, ChunkResidency::Spilled);
    ctl.ensure_resident(&mut registry, spilled_id).unwrap();
    assert_eq!(registry.get(spilled_id).unwrap().residency, ChunkResidency::Resident);
}

#[test]
fn checkpoint_resume_rejects_wrong_chunk_identity() {
    let heap = GcHeap::new(athena_gc::HeapBudget::default());
    let mut h = heap.borrow_mut();
    let mut registry = ChunkRegistry::new();
    let budget = MemoryBudget::new(64 * 1024).unwrap();
    let (published, _csr) = tiny_directed().finish_csr_on_heap(&mut h, &mut registry, budget).unwrap();
    let (mut checkpoint, _) = bind_algorithm_checkpoint(
        &published.publication,
        &mut h,
        FrontierCheckpoint { queue: vec![NodeId(0)], discovered: vec![true, false, false], visited_prefix: vec![] },
    )
    .unwrap();
    checkpoint.revision = GraphRevision(99);
    let err =
        resume_from_algorithm_checkpoint(published.graph.as_graph(), &published.publication, checkpoint, None).unwrap_err();
    assert!(matches!(err, GraphError::CheckpointIdentityMismatch));
}

#[test]
fn checkpoint_resume_does_not_need_raw_pointers() {
    let heap = GcHeap::new(athena_gc::HeapBudget::default());
    let mut h = heap.borrow_mut();
    let mut registry = ChunkRegistry::new();
    let budget = MemoryBudget::new(64 * 1024).unwrap();
    let (published, _csr) = tiny_directed().finish_csr_on_heap(&mut h, &mut registry, budget).unwrap();
    let mut cancel = CancelFlag::new();
    cancel.cancel();
    let first = athena_graph::deterministic_bfs(published.graph.as_graph(), NodeId(0), Some(&cancel)).unwrap();
    let DeterministicBfsOutcome::Cancelled { checkpoint: frontier, .. } = first
    else {
        panic!("expected cancelled");
    };
    let (checkpoint, _) = bind_algorithm_checkpoint(&published.publication, &mut h, frontier).unwrap();
    let resumed =
        resume_from_algorithm_checkpoint(published.graph.as_graph(), &published.publication, checkpoint, None).unwrap();
    let DeterministicBfsOutcome::Complete(order) = resumed
    else {
        panic!("expected complete");
    };
    assert_eq!(order, vec![NodeId(0), NodeId(1), NodeId(2)]);
}
