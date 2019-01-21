//! Scratch mark / rewind 合同。

use athena_gc::{HeapBudget, ScratchArena};

#[test]
fn rewind_restores_cursor_without_losing_capacity() {
    let mut scratch = ScratchArena::new();
    let budget = HeapBudget::default();
    scratch.ensure(4096, &budget, true).expect("ensure");
    let mark = scratch.mark();
    assert_eq!(scratch.used_bytes(), 0);
    let _ = scratch.allocate_limbs_zeroed(16, &budget).expect("alloc");
    assert!(scratch.used_bytes() >= 16 * 8);
    scratch.rewind(mark);
    assert_eq!(scratch.used_bytes(), 0);
    assert!(scratch.capacity_bytes() >= 4096);
}

#[test]
fn scratch_respects_byte_budget() {
    let mut scratch = ScratchArena::new();
    let budget = HeapBudget {
        max_scratch_bytes: 64,
        ..HeapBudget::default()
    };
    let err = scratch.ensure(128, &budget, true).expect_err("should fail");
    assert!(matches!(err, athena_gc::GcError::ScratchBytesLimit { .. }));
}
