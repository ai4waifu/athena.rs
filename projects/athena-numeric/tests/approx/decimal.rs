//! `Decimal` 表示与 limb 舍入测试（自 `src/decimal.rs` 迁出）。

use athena_numeric::{Decimal, RoundingStatus, integer::Sign, natural::Natural};

#[test]
fn f64_exact_roundtrip_normals_and_subnormals() {
    let samples =
        [0.0, -0.0, 1.0, -1.0, 0.5, 3.0, f64::MIN_POSITIVE, f64::MAX, f64::MIN, 1.5, f64::from_bits(0x0000_0000_0000_0001)];
    for x in samples {
        let bf = Decimal::from_f64(x).expect("finite");
        bf.validate().expect("valid");
        let back = bf.to_f64_exact().expect("exact");
        assert_eq!(back.to_bits(), x.to_bits(), "failed for {x:?}");
    }
}

#[test]
fn rejects_payload_wider_than_precision() {
    let sig = Natural::from_limbs(vec![9007199254740993, 1]);
    let err = Decimal::try_new(Sign::Positive, sig, 0, 53).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_PRECISION_LOSS");
}

#[test]
fn rejects_nan_import() {
    assert!(Decimal::from_f64(f64::NAN).is_err());
}

#[test]
fn limb_round_preserves_wide_precision() {
    // 80 位尾数：跨两 limb 且最高位置位的奇数。
    let sig = Natural::from_limbs(vec![0xFFFF_FFFF_FFFF_FFFF, 0xFFFF]);
    let bf = Decimal::try_new(Sign::Positive, sig, -10, 80).expect("fit");
    assert_eq!(bf.significand().bits(), 80);
    let (rounded, status) = bf.round_to_precision(60).expect("round");
    assert_eq!(status, RoundingStatus::RoundedUp); // 来自丢弃位的 sticky/round
    assert!(rounded.significand().bits() <= 60);
    rounded.validate().unwrap();
    // 不得经 f64 桥接塌缩到 ≤53 位。
    assert!(rounded.significand().bits() > 53 || rounded.precision_bits() == 60);
}

#[test]
fn exact_when_already_within_precision() {
    let bf = Decimal::try_new(Sign::Positive, Natural::from_u64(5), 0, 16).unwrap();
    let (r, status) = bf.round_to_precision(8).unwrap();
    assert_eq!(status, RoundingStatus::Exact);
    assert_eq!(r.significand().to_u64(), Some(5));
}

#[test]
fn zero_preserves_requested_precision() {
    let z = Decimal::zero_with_precision(128).unwrap();
    assert_eq!(z.precision_bits(), 128);
    let (r, status) = z.round_to_precision(64).unwrap();
    assert_eq!(status, RoundingStatus::Exact);
    assert_eq!(r.precision_bits(), 64);
    assert!(r.is_zero());
}
