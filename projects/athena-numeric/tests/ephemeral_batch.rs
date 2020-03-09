//! `TemporaryNatural` batch lease / promote 合同（需 feature `ephemeral`）。

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{ExecutionBudget, NumericContext, TemporaryNatural};

#[test]
fn temporary_natural_batch_lease_and_promote() {
    let batch_heap = GcHeap::new_shared(HeapBudget::default());
    let persist_heap = GcHeap::new_shared(HeapBudget::default());
    let persist_ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), persist_heap.clone());
    let mut h = batch_heap.borrow_mut();
    let used0: usize = h.segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum();
    let mut promoted = None;
    h.with_numeric_batch(|batch| {
        for _ in 0..16 {
            let block = batch.allocate_limbs(4).expect("alloc");
            let _ = block;
        }
        let n = TemporaryNatural::try_add(&[1, 2, 3, 4], &[5, 6, 7, 8], batch).expect("add");
        assert!(!n.is_zero());
        promoted = Some(n.promote(&persist_ctx).expect("promote"));
        drop(n);
    })
    .expect("batch");
    let used1: usize = h.segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum();
    assert_eq!(used1, used0, "batch rewind restores bump");
    assert_eq!(h.accounting(), athena_gc::AllocationAccounting::Full);
    assert!(!h.bump_ephemeral());
    let p = promoted.expect("promoted");
    assert_eq!(p.as_limbs(), &[6, 8, 10, 12]);
    assert!(persist_heap.borrow().resident_bytes() > 0);
}
