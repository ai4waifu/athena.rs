//! BigFloat and Real promotion tests.

use athena_numeric::{
    BigFloat, DefaultPromotion, NumericValue, PrecisionKind, PromotionPolicy, Real, integer::Sign, natural::Natural,
};

#[test]
fn machine_to_arbitrary_is_honest_53_bit_import() {
    let m = NumericValue::machine_real(1.5);
    let promoted = DefaultPromotion::promote_real_precision(m, PrecisionKind::Arbitrary, &PromotionPolicy::default())
        .expect("promote");
    match promoted.as_real() {
        Some(Real::BigFloat(b)) => {
            assert_eq!(b.precision_bits(), 53);
            assert_eq!(b.to_f64_exact(), Some(1.5));
        }
        other => panic!("expected BigFloat, got {other:?}"),
    }
}

#[test]
fn arbitrary_to_machine_requires_policy() {
    let bf = BigFloat::from_f64(1.25).unwrap();
    let v = NumericValue::big_float(bf);
    let err = DefaultPromotion::promote_real_precision(v, PrecisionKind::Machine, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn arbitrary_to_machine_roundtrip() {
    let bf = BigFloat::from_f64(core::f64::consts::PI).unwrap();
    let v = NumericValue::big_float(bf);
    let policy = PromotionPolicy { allow_exact_to_machine: false, allow_arbitrary_to_machine: true };
    let m = DefaultPromotion::promote_real_precision(v, PrecisionKind::Machine, &policy).unwrap();
    assert_eq!(m.as_machine_f64().unwrap().to_bits(), core::f64::consts::PI.to_bits());
}

#[test]
fn non_finite_machine_stays_machine_on_arbitrary_promote() {
    let nan = NumericValue::machine_real(f64::NAN);
    let kept = DefaultPromotion::promote_real_precision(nan, PrecisionKind::Arbitrary, &PromotionPolicy::default()).unwrap();
    assert!(matches!(kept.as_real(), Some(Real::Machine(x)) if x.is_nan()));
}

#[test]
fn normalized_invariant() {
    let bf = BigFloat::try_new(Sign::Positive, Natural::from_u64(8), 0, 8).unwrap();
    assert_eq!(bf.significand().to_u64(), Some(1));
    assert_eq!(bf.exponent(), 3);
}
