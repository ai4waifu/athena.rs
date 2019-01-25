//! `GcMode` 作用域 guard 合同。

use athena_gc::{GcHeap, GcMode, HeapBudget};

#[test]
fn suspend_overrides_to_disabled_and_restores() {
    let heap = GcHeap::new(HeapBudget::default());
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
    let heap = GcHeap::new(HeapBudget::default());
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
    let _g = a.suspend();
    assert_eq!(a.effective_mode(), GcMode::Disabled);
    assert_eq!(b.effective_mode(), GcMode::Auto);
}
