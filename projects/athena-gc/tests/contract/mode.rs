//! `GcMode` 作用域 guard 合同。

use athena_gc::{GcHeap, GcMode, HeapBudget};

#[test]
fn suspend_overrides_to_disabled_and_restores() {
    let rc = GcHeap::new(HeapBudget::default());
    let heap = rc.borrow();
    heap.gc().set_base_mode(GcMode::Auto);
    assert_eq!(heap.effective_mode(), GcMode::Auto);

    {
        let _g = heap.suspend();
        assert_eq!(heap.effective_mode(), GcMode::Disabled);
    }
    assert_eq!(heap.effective_mode(), GcMode::Auto);
}

#[test]
fn defer_overrides_and_nested_suspend_wins() {
    let rc = GcHeap::new(HeapBudget::default());
    let heap = rc.borrow();
    heap.gc().set_base_mode(GcMode::Auto);

    let _d = heap.defer();
    assert_eq!(heap.effective_mode(), GcMode::Deferred);
    {
        let _s = heap.suspend();
        assert_eq!(heap.effective_mode(), GcMode::Disabled);
    }
    assert_eq!(heap.effective_mode(), GcMode::Deferred);
}

#[test]
fn no_global_mode_leak_across_heaps() {
    let a = GcHeap::new(HeapBudget::default());
    let b = GcHeap::new(HeapBudget::default());
    let _g = a.borrow().suspend();
    assert_eq!(a.borrow().effective_mode(), GcMode::Disabled);
    assert_eq!(b.borrow().effective_mode(), GcMode::Auto);
}
