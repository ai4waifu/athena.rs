//! `Integer` owned / view 合同。

use athena_gc::{GcHeap, HeapBudget};
use athena_numeric::{ExecutionBudget, Integer, NumericContext, natural::Natural};

#[test]
fn div_zero_returns_diagnostic_without_panic() {
    let err = Integer::from_i64(1).div(&Integer::zero()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DIVISION_BY_ZERO");
}

#[test]
fn try_add_view_matches_try_add_on_wide_magnitudes() {
    let ctx = NumericContext::portable_default();
    let a = Integer::from_u64(u64::MAX).try_mul(&Integer::from_u64(u64::MAX), &ctx).unwrap();
    let b = Integer::from_u64(3).try_mul(&Integer::from_u64(u64::MAX), &ctx).unwrap();
    let via_ref = a.try_add(&b, &ctx).unwrap();
    let via_view = Integer::try_add_view(a.magnitude_view(), b.magnitude_view(), &ctx).unwrap();
    assert_eq!(via_ref, via_view);
    assert!(via_ref.bits() > 64);
}

#[test]
fn try_add_owned_same_sign_matches_try_add() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mag = Natural::from_limbs_with_capacity_in(&ctx, &[1, 2, 3, 4], 8).expect("mag");
    let a = Integer::from_natural_sign(mag, true);
    let b = Integer::from_natural_sign(Natural::from_limbs_in(&ctx, vec![5, 6, 7]).expect("b"), true);
    let expected = a.try_add(&b, &ctx).expect("ref");
    let sum = a.try_add_owned(&b, &ctx).expect("owned");
    assert_eq!(sum, expected);
    assert!(sum.is_negative());
}

#[test]
fn try_mul_u64_owned_preserves_sign() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    let mag = Natural::from_limbs_with_capacity_in(&ctx, &[3, 4, 5], 6).expect("mag");
    let a = Integer::from_natural_sign(mag, true);
    let expected = a.try_mul(&Integer::from_i64(-7), &ctx).expect("ref");
    let prod = a.try_mul_u64_owned(7, &ctx).expect("owned");
    assert_eq!(prod.abs(), expected.abs());
    assert!(prod.is_negative());
}
