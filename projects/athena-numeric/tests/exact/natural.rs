//! `Natural`（无符号大整数）算术与 limb 内核测试。

use athena_numeric::{NumericContext, Integer, NumericValue, natural::Natural, number_from_wire};
use athena_types::wire::{ExactNumber, WireNumber};
use std::str::FromStr;

fn int_from_wire(s: &str) -> Integer {
    match number_from_wire(&WireNumber::Exact(ExactNumber::Integer(s.to_string()))).unwrap() {
        NumericValue::Integer(i) => i.try_clone_in(&NumericContext::portable_default()).unwrap(),
        other => panic!("expected integer, got {other:?}"),
    }
}

#[test]
fn parse_and_arith() {
    let a = Natural::from_str("12345").unwrap();
    let b = Natural::from_str("67890").unwrap();
    assert_eq!(a.add(&b).to_decimal_string(), "80235");
    assert_eq!(b.sub(&a).to_decimal_string(), "55545");
    assert_eq!(a.mul_u64(10).to_decimal_string(), "123450");
    let (q, r) = Natural::from_str("17").unwrap().div_rem(&Natural::from_u64(5));
    assert_eq!(q.to_decimal_string(), "3");
    assert_eq!(r.to_decimal_string(), "2");
}

#[test]
fn mod_pow_smoke() {
    let base = Natural::from_u64(3);
    let exp = Natural::from_u64(4);
    let m = Natural::from_u64(7);
    assert_eq!(base.mod_pow(&exp, &m).to_decimal_string(), "4");
}

#[test]
fn math_ord_two_pow_64_gt_u64_max() {
    let two_pow_64 = Natural::from_str("18446744073709551616").unwrap();
    let u64_max = Natural::from_u64(u64::MAX);
    assert!(two_pow_64 > u64_max);
    assert!(u64_max < two_pow_64);
}

#[test]
fn div_rem_identity_large() {
    let cases = [("12345", "17"), ("99999999999999999999", "123456789"), ("100000000000000000000", "99999")];
    for (a, b) in cases {
        let na = Natural::from_str(a).unwrap();
        let nb = Natural::from_str(b).unwrap();
        let (q, r) = na.div_rem(&nb);
        let rebuilt = q.mul(&nb).add(&r);
        assert_eq!(rebuilt, na, "identity failed for {a}/{b}");
        assert!(r < nb);
    }
}

#[test]
fn karatsuba_path_large_mul() {
    // 约 640 位十进制数字 → >32 limb，迫使 limb 内核走 Karatsuba。
    let digits = "9876543210".repeat(64);
    let a = Natural::from_str(&digits).unwrap();
    let b = Natural::from_str("8888888888888888888888888888888888888888888888888888888888888888").unwrap();
    let prod = a.mul(&b);
    let (q, r) = prod.div_rem(&a);
    assert_eq!(r, Natural::zero());
    assert_eq!(q, b);
}

#[test]
fn gcd_matches_euclidean_reference() {
    let pairs = [("48", "18"), ("270", "192"), ("99999999999999999999", "12345678901234567890")];
    for (a, b) in pairs {
        let x = Natural::from_str(a).unwrap();
        let y = Natural::from_str(b).unwrap();
        let g = reference_gcd(x, y);
        let gi = int_from_wire(a).gcd(&int_from_wire(b));
        assert_eq!(gi.to_decimal_string(), g.to_decimal_string());
    }
}

fn reference_gcd(mut x: Natural, mut y: Natural) -> Natural {
    while !y.is_zero() {
        let (_, r) = x.div_rem(&y);
        x = y;
        y = r;
    }
    x
}
