//! destination reuse / `try_reuse_unique_published` / `UniqueMutationGuard` 合同。

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{CapabilityBundle, ExecutionBudget, Integer, NumericContext, ResourceCapability, natural::Natural};

#[test]
fn can_reuse_destination_false_still_matches_add() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let mut caps = CapabilityBundle::portable_default();
    caps.resource = ResourceCapability { can_reuse_destination: false, ..caps.resource };
    let ctx = NumericContext::with_capabilities(ExecutionBudget::unlimited(), heap, caps);
    assert!(!ctx.can_reuse_destination());

    let a = Natural::from_limbs_in(&ctx, vec![1, 2, 3, 4]).expect("a");
    let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
    let via_reuse_off = a.try_add(&b, &ctx).expect("add");
    let via_owned = a.try_clone_in(&NumericContext::portable_default()).unwrap().try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(via_reuse_off.as_limbs(), via_owned.as_limbs());
}

#[test]
fn spare_capacity_add_owned_reuses_unique_published_block() {
    use core::ptr::NonNull;

    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[1, 2, 3, 4], 16).expect("a");
    let ptr_before = a.as_limbs().as_ptr();
    let b = Natural::from_u64(9);
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
    // 唯一 published 块原地复用，指针不变且仍为 TracingSweep。
    assert_eq!(sum.as_limbs().as_ptr(), ptr_before);
    let nn = NonNull::new(sum.as_limbs().as_ptr() as *mut u64).expect("ptr");
    assert!(ctx.heap().borrow().may_root_numeric(nn).expect("still published"));
    assert!(!ctx.heap().borrow().may_explicit_release_numeric(nn).expect("not temp"));
}

#[test]
fn published_add_owned_falls_back_when_capacity_tight() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    // Tight capacity：无法 reuse 唯一 published 块时回退 try_add，结果仍正确。
    let a = Natural::from_limbs_in(&ctx, vec![u64::MAX, u64::MAX, u64::MAX]).expect("a");
    let b = Natural::from_u64(1);
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
}

#[test]
fn natural_self_add_owned_aliases_safely() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[9, 8, 7, 6], 10).expect("a");
    let expected = a.try_add(&a, &ctx).expect("ref");
    let sum = a.try_clone_in(&NumericContext::portable_default()).unwrap().try_add_owned(&a, &ctx).expect("owned self");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
}

#[test]
fn integer_opposite_sign_add_owned_matches_try_add() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mag_a = Natural::from_limbs_with_capacity_in(&ctx, &[10, 20, 30, 40], 8).expect("a");
    let mag_b = Natural::from_limbs_in(&ctx, vec![1, 2, 3]).expect("b");
    let a = Integer::from_natural_sign(mag_a, false);
    let b = Integer::from_natural_sign(mag_b, true);
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum, expected);
}

#[test]
fn integer_try_sub_owned_matches_try_sub() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mag = Natural::from_limbs_with_capacity_in(&ctx, &[50, 60, 70], 8).expect("mag");
    let a = Integer::from_natural_sign(mag, true);
    let b = Integer::from_natural_sign(Natural::from_limbs_in(&ctx, vec![1, 2, 3]).expect("b"), false);
    let expected = a.try_sub(&b, &ctx).expect("ref");
    let diff = a.try_sub_owned(&b, &ctx).expect("owned");
    assert_eq!(diff, expected);
}

#[test]
fn integer_try_mul_owned_matches_try_mul() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mag = Natural::from_limbs_with_capacity_in(&ctx, &[3, 4, 5], 8).expect("mag");
    let a = Integer::from_natural_sign(mag, true);
    let b = Integer::from_natural_sign(Natural::from_limbs_in(&ctx, vec![7, 8]).expect("b"), true);
    let expected = a.try_mul(&b, &ctx).expect("ref");
    let prod = a.try_mul_owned(&b, &ctx).expect("owned");
    assert_eq!(prod, expected);
    assert!(!prod.is_negative());
}

#[test]
fn spare_capacity_add_owned_matches_try_add() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[1, 2, 3, 4], 16).expect("a");
    let b = Natural::from_u64(9);
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
}
