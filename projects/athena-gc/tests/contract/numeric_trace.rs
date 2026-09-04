//! GC-owned numeric tracing reclaim 与 scratch promote。

use athena_gc::{EmptyObjectGraph, GcHeap, GcMode, HeapBudget, RootKind};

#[test]
fn traced_numeric_swept_when_unreachable() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Deferred);

    let kept = heap.allocate_traced_numeric(4).expect("kept");
    let orphan = heap.allocate_traced_numeric(4).expect("orphan");
    let token = heap.roots_mut().register_numeric(kept.ptr.cast(), RootKind::Numeric);

    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect");
    assert!(report.numeric_blocks_swept >= 1);
    assert!(heap.header_for_limbs(kept.ptr).is_ok());
    assert!(heap.header_for_limbs(orphan.ptr).is_err());

    assert!(heap.roots_mut().unregister_numeric(token));
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect2");
    assert!(report.numeric_blocks_swept >= 1);
    assert!(heap.header_for_limbs(kept.ptr).is_err());
}

#[test]
fn promote_limbs_creates_traced_block() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Deferred);

    let src = [7u64, 8, 9];
    let block = heap.promote_limbs(&src).expect("promote");
    let view = heap.published_limbs(&block).expect("view");
    assert_eq!(&view[..3], &src);

    let token = heap.roots_mut().register_numeric(block.ptr.cast(), RootKind::Numeric);
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect");
    assert_eq!(report.numeric_blocks_swept, 0);
    assert!(heap.roots_mut().unregister_numeric(token));
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect2");
    assert!(report.numeric_blocks_swept >= 1);
}

#[test]
fn temporary_numeric_not_swept_by_tracing() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Deferred);
    let block = heap.allocate_numeric_block(4).expect("temporary");
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect");
    assert_eq!(report.numeric_blocks_swept, 0);
    assert!(heap.header_for_limbs(block.ptr).is_ok());
    heap.release_numeric_block(block).expect("rel");
}
