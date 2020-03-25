//! reclaim authority / typed handles / explicit release 合同（非 ownership 实体）。

use std::{cell::RefCell, rc::Rc};

use athena_gc::{GcError, GcHeap, HeapBudget, ReclaimAuthority, RootKind};

fn with_heap<R>(f: impl FnOnce(Rc<RefCell<GcHeap>>, &mut GcHeap) -> R) -> R {
    let rc = GcHeap::new(HeapBudget::default());
    let mut borrow = rc.borrow_mut();
    f(rc.clone(), &mut borrow)
}

#[test]
fn temporary_block_rejects_root_and_accepts_release() {
    with_heap(|_rc, heap| {
        let block = heap.allocate_numeric_block(4).expect("temp");
        assert!(heap.may_explicit_release_numeric(block.ptr).expect("temp"));
        assert!(!heap.may_root_numeric(block.ptr).expect("not published"));
        assert_eq!(heap.header_for_limbs(block.ptr).expect("hdr").reclaim_authority, ReclaimAuthority::ExplicitRelease);
        assert!(matches!(heap.register_numeric_root_ptr(block.ptr, RootKind::Session), Err(GcError::LifecycleMismatch)));
        heap.release_numeric_block(block).expect("release");
    });
}

#[test]
fn published_block_accepts_root_and_is_not_explicit_release() {
    with_heap(|_rc, heap| {
        let block = heap.allocate_traced_numeric(4).expect("published");
        assert!(heap.may_root_numeric(block.ptr).expect("rootable"));
        assert!(!heap.may_explicit_release_numeric(block.ptr).expect("not temp"));
        assert_eq!(heap.header_for_limbs(block.ptr).expect("hdr").reclaim_authority, ReclaimAuthority::TracingSweep);
        let _token = heap.register_numeric_root(&block, RootKind::Session).expect("root");
        // `release_numeric_block` 只接受 `TemporaryNumericBlock`，类型上无法对 published 调用。
        let _ = block;
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
    GcHeap::release_temporary_numeric_registered(block.heap_id, block.ptr).expect("drop path");
}
