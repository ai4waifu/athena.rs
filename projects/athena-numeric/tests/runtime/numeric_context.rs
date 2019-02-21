//! N3 `NumericContext`：cancel 与统一 allocator / scratch 钩子。

use athena_gc::HeapBudget;
use athena_numeric::{CancellationToken, ExecutionBudget, NumericContext, natural::Natural};
use athena_types::DiagnosticCode;

#[test]
fn cancel_rejects_try_add_before_work() {
    let token = CancellationToken::new();
    token.cancel();
    let ctx = NumericContext::unlimited().with_cancellation(token);
    let a = Natural::from_u64(1);
    let b = Natural::from_u64(2);
    let err = a.try_add(&b, &ctx).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::NumericCancelled);
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CANCELLED");
}

#[test]
fn cancel_token_shared_across_contexts() {
    let token = CancellationToken::new();
    let ctx_a = NumericContext::unlimited().with_cancellation(token.clone());
    let ctx_b = NumericContext::unlimited().with_cancellation(token);
    assert!(!ctx_a.is_cancelled());
    ctx_b.cancel();
    assert!(ctx_a.is_cancelled());
    let err = Natural::from_u64(3).try_mul(&Natural::from_u64(4), &ctx_a).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::NumericCancelled);
}

#[test]
fn allocate_numeric_block_respects_cancel() {
    let ctx = NumericContext::unlimited();
    ctx.cancel();
    let err = ctx.allocate_numeric_block(4).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::NumericCancelled);
}

#[test]
fn allocate_numeric_block_on_isolated_heap() {
    let ctx = NumericContext::with_new_heap(ExecutionBudget::unlimited(), HeapBudget::default());
    let block = ctx.allocate_numeric_block(8).expect("alloc");
    assert!(block.capacity >= 8);
}

#[test]
fn with_scratch_frame_clears_cursor() {
    let ctx = NumericContext::unlimited();
    ctx.with_scratch_frame(|scratch, budget| {
        scratch.ensure(8, budget).expect("ensure");
        let slot = scratch.alloc(4).expect("alloc");
        slot[0] = 9;
        assert_eq!(scratch.mark(), 4);
    });
    ctx.with_scratch(|scratch| {
        assert_eq!(scratch.mark(), 0);
    });
}

#[test]
fn with_gc_scratch_rewinds_to_entry_mark() {
    let ctx = NumericContext::with_new_heap(ExecutionBudget::unlimited(), HeapBudget::default());
    let budget = HeapBudget::default();
    let used_after = ctx.with_gc_scratch(|arena| {
        arena.ensure(256, &budget, true).expect("ensure");
        arena.allocate_uninit(48, &budget).expect("bump");
        arena.used_bytes()
    });
    assert!(used_after >= 48);
    // 帧结束已 rewind：再次 mark 的 used 应回到进入前（0）。
    let restored = ctx.with_gc_scratch(|arena| arena.used_bytes());
    assert_eq!(restored, 0);
}

#[test]
fn heap_try_mul_respects_cancel_at_entry() {
    let limbs = vec![u64::MAX; 8];
    let a = Natural::from_limbs(limbs.clone()).unwrap();
    let b = Natural::from_limbs(limbs).unwrap();
    let ctx = NumericContext::unlimited();
    ctx.cancel();
    let err = a.try_mul(&b, &ctx).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::NumericCancelled);
}
