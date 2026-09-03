//! `Natural` owned 路径与符号 don't-care 合同。

use std::{cmp::Ordering, collections::HashSet};

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{ExecutionBudget, Integer, NumericContext, natural::Natural};

#[test]
fn natural_sign_bit_is_semantic_dont_care() {
    let a = Natural::from_u64(42);
    let b = a.try_clone_in(&NumericContext::portable_default()).unwrap().with_dont_care_sign_bit(true);
    assert_eq!(a, b);
    assert_eq!(a.cmp(&b), Ordering::Equal);

    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

#[test]
fn integer_interprets_sign_bit() {
    assert_ne!(Integer::from_u64(42), Integer::from_i64(-42));
}

#[test]
fn try_add_owned_matches_try_add_with_spare_capacity() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[1, 2, 3, 4], 8).expect("a");
    let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
}

#[test]
fn try_add_owned_matches_try_add_without_spare_capacity() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_in(&ctx, vec![u64::MAX, u64::MAX, u64::MAX]).expect("a");
    let b = Natural::from_u64(1);
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
}

#[test]
fn try_mul_u64_owned_matches_try_mul_u64_with_spare_capacity() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[3, 4, 5], 6).expect("a");
    let expected = a.try_mul_u64(7, &ctx).expect("ref");
    let prod = a.try_mul_u64_owned(7, &ctx).expect("owned");
    assert_eq!(prod.as_limbs(), expected.as_limbs());
}

#[test]
fn try_add_owned_self_add_aliases_safely() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[9, 8, 7], 8).expect("a");
    let expected = a.try_add(&a, &ctx).expect("ref");
    let sum = a.try_clone_in(&NumericContext::portable_default()).unwrap().try_add_owned(&a, &ctx).expect("owned self");
    assert_eq!(sum.as_limbs(), expected.as_limbs());
}

#[test]
fn try_sub_owned_matches_try_sub_with_spare_capacity() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[10, 20, 30, 40], 6).expect("a");
    let b = Natural::from_limbs_in(&ctx, vec![1, 2, 3]).expect("b");
    let expected = a.try_sub(&b, &ctx).expect("ref");
    let diff = a.try_sub_owned(&b, &ctx).expect("owned");
    assert_eq!(diff.as_limbs(), expected.as_limbs());
}

#[test]
fn try_mul_owned_matches_try_mul_with_spare_capacity() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_with_capacity_in(&ctx, &[2, 3, 4], 8).expect("a");
    let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
    let expected = a.try_mul(&b, &ctx).expect("ref");
    let prod = a.try_mul_owned(&b, &ctx).expect("owned");
    assert_eq!(prod.as_limbs(), expected.as_limbs());
}

#[test]
fn try_mul_owned_falls_back_without_spare_capacity() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let a = Natural::from_limbs_in(&ctx, vec![2, 3, 4]).expect("a");
    let b = Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b");
    let expected = a.try_mul(&b, &ctx).expect("ref");
    let prod = a.try_mul_owned(&b, &ctx).expect("owned");
    assert_eq!(prod.as_limbs(), expected.as_limbs());
}
