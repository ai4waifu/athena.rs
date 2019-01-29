//! Heap → `athena-gc` 分配与 Trace 合同。

use athena_gc::{EmptyObjectGraph, GcHeap, GcMode, HeapBudget, MarkState, RootKind, Trace, Tracer};
use athena_numeric::{NumericContext, execution_budget::ExecutionBudget, natural::Natural};

struct CaptureAllocs {
    marked: Vec<usize>,
}

impl Tracer for CaptureAllocs {
    fn mark_object(&mut self, _id: athena_gc::GcObjectId) {}

    fn mark_allocation(&mut self, payload: *const u8) {
        self.marked.push(payload as usize);
    }
}

#[test]
fn heap_natural_allocates_on_ctx_heap_and_traces() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    {
        let h = heap.borrow_mut();
        h.gc().set_base_mode(GcMode::Deferred);
    }
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap.clone());

    // ≥3 limbs → Heap magnitude
    let limbs = vec![1u64, 2, 3, 4];
    let n = Natural::from_limbs_in(&ctx, limbs).expect("alloc");
    let mut cap = CaptureAllocs { marked: Vec::new() };
    n.trace(&mut cap);
    assert!(!cap.marked.is_empty(), "Heap Natural must mark limb allocation");

    let before = heap.borrow().resident_bytes();
    assert!(before > 0);
    drop(n);
    let report = heap.borrow_mut().collect().expect("collect");
    assert!(report.segments_reclaimed >= 1 || heap.borrow().resident_bytes() < before);
}

#[test]
fn isolated_heap_not_shared_default() {
    let heap_a = GcHeap::new_shared(HeapBudget::default());
    let heap_b = GcHeap::new_shared(HeapBudget::default());
    assert_ne!(heap_a.borrow().id(), heap_b.borrow().id());

    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap_a.clone());
    let n = Natural::from_limbs_in(&ctx, vec![9, 8, 7]).expect("alloc");
    drop(n);
    let _ = heap_a.borrow_mut().collect();
    assert_eq!(heap_b.borrow().resident_bytes(), 0);
}

#[test]
fn object_root_keeps_payload_across_collect() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Deferred);
    let obj = heap.allocate_object(16).expect("obj");
    heap.object_payload_mut(obj).expect("w")[0] = 7;
    let token = heap.roots_mut().register(obj, RootKind::UserRetain);
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect");
    assert_eq!(report.objects_swept, 0);
    assert_eq!(heap.object_payload(obj).expect("ro")[0], 7);
    assert!(heap.roots_mut().unregister(token));
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect2");
    assert!(report.objects_swept >= 1);
    assert!(heap.object_payload(obj).is_err());
}

#[test]
fn mark_state_black_after_mark_limbs() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    let block = heap.allocate_numeric_block(4).expect("block");
    heap.mark_limbs(block.ptr).expect("mark");
    let hdr = heap.header_for_limbs(block.ptr).expect("hdr");
    assert_eq!(hdr.mark_state, MarkState::Black);
    heap.release_numeric_block(block).expect("rel");
}
