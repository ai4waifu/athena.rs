//! 数组简版 GC：真实 `allocate_object` + Trace + `RootKind::Array`。

use athena_gc::{GcHeap, HeapBudget, Trace};
use athena_ndarray::{ArrayId, ArrayLayout, ArrayRevision, LogicalShape, RecordingTracer, allocate_array_chunk_id, finish_array_on_heap};

#[test]
fn finish_array_on_heap_registers_roots() {
    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let shape = LogicalShape::new([3, 2]).unwrap();
    let layout = ArrayLayout::row_major(shape.clone(), 8).unwrap();
    let published = finish_array_on_heap(&mut h, shape, layout).unwrap();
    assert!(h.object_payload(published.snapshot_id().as_object()).is_ok());
    assert!(h.object_payload(published.publication.revision_id.as_object()).is_ok());
    assert_eq!(published.publication.chunks.len(), 1);
    let index = published.publication.trace_index();
    h.collect_traced(&index).unwrap();
    assert!(h.object_payload(published.snapshot_id().as_object()).is_ok());
}

#[test]
fn array_id_is_not_shape() {
    let a = ArrayId::allocate();
    let b = ArrayId::allocate();
    assert_ne!(a, b);
    let shape = LogicalShape::new([2, 2]).unwrap();
    assert_ne!(a.0, shape.element_count());
}

#[test]
fn snapshot_trace_marks_revision_and_chunks() {
    let heap = GcHeap::new(HeapBudget::default());
    let mut h = heap.borrow_mut();
    let shape = LogicalShape::new([4]).unwrap();
    let layout = ArrayLayout::row_major(shape.clone(), 8).unwrap();
    let chunk = allocate_array_chunk_id(&mut h).unwrap();
    let published = athena_ndarray::publish_array_snapshot(&mut h, ArrayId::allocate(), ArrayRevision(2), shape, layout, vec![chunk]).unwrap();
    let mut tracer = RecordingTracer::default();
    published.publication.snapshot_record.trace(&mut tracer);
    assert!(tracer.marked.contains(&published.snapshot_id().as_object()));
    assert!(tracer.marked.contains(&published.publication.revision_id.as_object()));
    assert!(tracer.marked.contains(&chunk.as_object()));
}
