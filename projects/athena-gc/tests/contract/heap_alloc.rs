//! Numeric block 分配 / pin / collect 合同。

use std::{cell::RefCell, rc::Rc};

use athena_gc::{BlockKind, GcHeap, GcMode, HeapBudget, RootKind};

fn with_heap<R>(f: impl FnOnce(Rc<RefCell<GcHeap>>, &mut GcHeap) -> R) -> R {
    let rc = GcHeap::new(HeapBudget::default());
    let mut borrow = rc.borrow_mut();
    // SAFETY-ish: 不在闭包内再 borrow rc。
    f(rc.clone(), &mut borrow)
}

#[test]
fn allocate_header_roundtrip_and_release_reclaim() {
    with_heap(|_rc, heap| {
        heap.gc().set_base_mode(GcMode::Auto);

        let block = heap.allocate_numeric_block(8).expect("alloc");
        assert_eq!(block.capacity, 8);
        let hdr = heap.header_for_limbs(block.ptr).expect("header");
        assert_eq!(hdr.block_kind, BlockKind::Numeric);
        assert_eq!(hdr.segment_id, block.segment_id);
        assert_eq!(hdr.heap_id, block.heap_id);

        {
            let limbs = heap.numeric_limbs_mut(&block).expect("limbs");
            for (i, slot) in limbs.iter_mut().enumerate() {
                *slot = i as u64 + 1;
            }
        }
        assert_eq!(heap.numeric_limbs(&block).expect("ro")[0], 1);

        assert!(heap.resident_bytes() > 0);
        heap.release_numeric_block(block).expect("release");
        assert_eq!(heap.resident_bytes(), 0);
        assert_eq!(heap.segments().count(), 0);
    });
}

#[test]
fn disabled_collect_does_not_reclaim_empty_segment() {
    with_heap(|_rc, heap| {
        heap.gc().set_base_mode(GcMode::Deferred);
        let block = heap.allocate_numeric_block(4).expect("alloc");
        heap.release_numeric_block(block).expect("release");
        assert!(heap.resident_bytes() > 0, "deferred release keeps empty segment");

        let _g = heap.suspend();
        let report = heap.collect().expect("collect");
        assert_eq!(report.mode, GcMode::Disabled);
        assert_eq!(report.segments_reclaimed, 0);
        assert!(heap.resident_bytes() > 0);
        drop(_g);

        let report = heap.collect().expect("collect after resume");
        assert!(report.segments_reclaimed >= 1 || heap.resident_bytes() == 0);
    });
}

#[test]
fn pin_blocks_reclaim() {
    with_heap(|_rc, heap| {
        heap.gc().set_base_mode(GcMode::Auto);
        let block = heap.allocate_numeric_block(4).expect("alloc");
        let seg = block.segment_id;
        {
            let _pin = heap.pin(&[seg]);
            heap.release_numeric_block(block).expect("release");
            assert!(heap.resident_bytes() > 0, "pinned empty segment must stay");
        }
        let _ = heap.collect().expect("collect");
        assert_eq!(heap.resident_bytes(), 0);
    });
}

#[test]
fn arena_budget_rejects_oversized_request() {
    let budget = HeapBudget { max_arena_bytes: 1024, max_segment_count: 2, max_limbs: 1_000_000, max_scratch_bytes: 1024 };
    let rc = GcHeap::new(budget);
    let err = rc.borrow_mut().allocate_numeric_block(8).expect_err("budget");
    assert!(matches!(err, athena_gc::GcError::ArenaBytesLimit { .. }));
}

#[test]
fn root_registry_register_unregister() {
    with_heap(|_rc, heap| {
        let id = athena_gc::GcObjectId { index: 1, generation: 1 };
        let token = heap.roots_mut().register(id, RootKind::Session);
        assert_eq!(heap.roots().len(), 1);
        assert!(heap.roots_mut().unregister(token));
        assert!(heap.roots().is_empty());
    });
}

#[test]
fn defer_records_pressure_without_auto_reclaim_on_alloc() {
    let budget = HeapBudget { max_arena_bytes: 64 * 1024 * 1024, ..HeapBudget::default() };
    let rc = GcHeap::new(budget);
    let mut heap = rc.borrow_mut();
    heap.gc().set_auto_threshold_bytes(1);
    let _d = heap.defer();
    let block = heap.allocate_numeric_block(4).expect("alloc");
    assert!(heap.gc().pressure().threshold_hit);
    assert!(heap.resident_bytes() > 0);
    heap.release_numeric_block(block).expect("release");
    drop(_d);
    let report = heap.collect().expect("collect");
    assert!(report.segments_reclaimed >= 1 || heap.resident_bytes() == 0);
}

#[test]
fn registered_release_via_heap_id() {
    let rc = GcHeap::new(HeapBudget::default());
    let block = {
        let mut heap = rc.borrow_mut();
        heap.gc().set_base_mode(GcMode::Deferred);
        heap.allocate_numeric_block(4).expect("alloc")
    };
    GcHeap::release_temporary_numeric_registered(block.heap_id, block.ptr).expect("release");
    let mut heap = rc.borrow_mut();
    let report = heap.collect().expect("collect");
    assert!(report.segments_reclaimed >= 1 || heap.resident_bytes() == 0);
}
