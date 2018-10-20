//! Integer arithmetic, ordering, and boundary conversion tests.

use std::cmp::Ordering;

use athena_numeric::{Integer, NumericValue, Sign, number_from_wire};
use athena_types::wire::{ExactNumber, WireNumber};

fn int_from_wire(s: &str) -> Integer {
    match number_from_wire(&WireNumber::Exact(ExactNumber::Integer(s.to_string()))).unwrap() {
        NumericValue::Integer(i) => i.clone(),
        other => panic!("expected integer, got {other:?}"),
    }
}

#[test]
fn div_rem_mod_pow_bits() {
    let a = Integer::from_i64(17);
    let b = Integer::from_i64(5);
    assert_eq!(a.div(&b), Integer::from_i64(3));
    assert_eq!(a.rem(&b), Integer::from_i64(2));
    assert!(a.is_positive());
    assert!(Integer::one().is_one());
    assert_eq!(Integer::from_i64(8).bits(), 4);
    let m = Integer::from_i64(7);
    assert_eq!(Integer::from_i64(3).mod_pow(&Integer::from_i64(4), &m), Integer::from_i64(4));
    assert_eq!(Integer::from_u64(42).to_u64(), Some(42));
    assert_eq!(Integer::from_i64(-1).to_u64(), None);
    let big = int_from_wire("99999999999999999999");
    assert_eq!(big.to_decimal_string(), "99999999999999999999");
}

#[test]
fn pow_no_recursion_and_large_exponent() {
    let two = Integer::from_i64(2);
    assert_eq!(two.pow_u32(0).unwrap(), Integer::one());
    assert_eq!(two.pow_u32(1).unwrap(), two);
    assert_eq!(two.pow_u32(32).unwrap(), int_from_wire("4294967296"));
    let exp33 = Integer::from_u64(33);
    assert_eq!(two.pow(&exp33).unwrap(), int_from_wire("8589934592"));
    let exp_over = Integer::from_i64(Integer::MAX_POW_EXP + 1);
    assert!(two.pow(&exp_over).is_err());
}

#[test]
fn try_to_f64_exact_respects_2_53() {
    assert_eq!(Integer::from_i64(1).try_to_f64_exact(), Some(1.0));
    let ok = Integer::from_i64(9_007_199_254_740_992); // 2^53
    assert_eq!(ok.try_to_f64_exact(), Some(9_007_199_254_740_992.0));
    let bad = Integer::from_i64(9_007_199_254_740_993);
    assert_eq!(bad.try_to_f64_exact(), None);
    assert!(bad.to_f64_approximate().is_some());
}

#[test]
fn gcd_zero_zero() {
    assert_eq!(Integer::zero().gcd(&Integer::zero()), Integer::zero());
}

#[test]
fn math_ord_negatives_and_antisymmetry() {
    assert!(Integer::from_i64(-2) < Integer::from_i64(-1));
    assert!(Integer::from_i64(-1) < Integer::zero());
    assert!(Integer::zero() < Integer::from_i64(1));
    assert!(Integer::from_i64(1) < Integer::from_i64(2));
    for (a, b) in [(-5_i64, -3), (-1, 0), (0, 4), (-2, 2), (7, 7)] {
        let x = Integer::from_i64(a);
        let y = Integer::from_i64(b);
        assert_eq!(x.cmp(&y), y.cmp(&x).reverse());
        let diff = x.sub(&y);
        let from_sign = match diff.sign() {
            Sign::Negative => Ordering::Less,
            Sign::Zero => Ordering::Equal,
            Sign::Positive => Ordering::Greater,
        };
        assert_eq!(x.cmp(&y), from_sign, "signum(a-b) vs cmp for {a},{b}");
    }
    assert!(Integer::from_i64(-10) < Integer::from_i64(-1) && Integer::from_i64(-1) < Integer::from_i64(5));
    assert!(Integer::from_i64(-10) < Integer::from_i64(5));
}

#[test]
fn boundary_i64_roundtrip() {
    for n in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
        assert_eq!(Integer::from_i64(n).to_i64(), Some(n), "roundtrip {n}");
    }
    let too_low = Integer::from_i64(i64::MIN).sub(&Integer::one());
    assert_eq!(too_low.to_i64(), None);
    let too_high = Integer::from_i64(i64::MAX).add(&Integer::one());
    assert_eq!(too_high.to_i64(), None);
}

#[test]
fn boundary_u64_zero_and_extrema() {
    assert_eq!(Integer::zero().to_u64(), Some(0));
    assert_eq!(Integer::zero().to_u128(), Some(0));
    assert_eq!(Integer::from_u64(0).to_u64(), Some(0));
    assert_eq!(Integer::from_u64(u64::MAX).to_u64(), Some(u64::MAX));
    assert_eq!(Integer::from_i64(-1).to_u64(), None);
}

#[test]
fn boundary_i64_min_distinct_from_overflow() {
    let min = Integer::from_i64(i64::MIN);
    assert_eq!(min.to_i64(), Some(i64::MIN));
    assert_eq!(min.to_decimal_string(), i64::MIN.to_string());
    assert_eq!(min.add(&Integer::one()).to_i64(), Some(i64::MIN + 1));
}
