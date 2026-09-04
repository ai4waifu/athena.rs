//! Living `24`：reclaim authority / root / explicit release 合同（非 ownership 实体）。

use std::{cell::RefCell, rc::Rc};

use athena_gc::{GcError, GcHeap, HeapBudget, ReclaimAuthority, RootKind};

fn with_heap<R>(f: impl FnOnce(Rc<RefCell<GcHeap>>, &mut GcHeap) -> R) -> R {
    let rc = GcHeap::new(HeapBudget::default());
    let mut borrow = rc.borrow_mut();
    f(rc.clone(), &mut borrow)
}

#[test]
fn explicit_release_block_rejects_root_and_accepts_release() {
    with_heap(|_rc, heap| {
        let block = heap.allocate_numeric_block(4).expect("temp");
        assert!(heap.may_explicit_release_numeric(block.ptr).expect("temp"));
        assert!(!heap.may_root_numeric(block.ptr).expect("not published"));
        assert_eq!(heap.header_for_limbs(block.ptr).expect("hdr").reclaim_authority, ReclaimAuthority::ExplicitRelease);
        assert!(matches!(heap.register_numeric_root(block.ptr, RootKind::Session), Err(GcError::LifecycleMismatch)));
        heap.release_numeric_block(block).expect("release");
    });
}

#[test]
fn tracing_sweep_block_accepts_root_and_rejects_explicit_release() {
    with_heap(|_rc, heap| {
        let block = heap.allocate_traced_numeric(4).expect("published");
        assert!(heap.may_root_numeric(block.ptr).expect("rootable"));
        assert!(!heap.may_explicit_release_numeric(block.ptr).expect("not temp"));
        assert_eq!(heap.header_for_limbs(block.ptr).expect("hdr").reclaim_authority, ReclaimAuthority::TracingSweep);
        let _token = heap.register_numeric_root(block.ptr, RootKind::Session).expect("root");
        assert!(matches!(heap.release_numeric_block(block), Err(GcError::LifecycleMismatch)));
    });
}

#[test]
fn may_explicit_release_registered_matches_local() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let block = {
        let mut h = heap.borrow_mut();
        h.allocate_numeric_block(2).expect("temp")
    };
    let ok = GcHeap::may_explicit_release_numeric_registered(block.heap_id, block.ptr).expect("query");
    assert!(ok);
    GcHeap::release_numeric_limbs_registered(block.heap_id, block.ptr).expect("drop path");
}
