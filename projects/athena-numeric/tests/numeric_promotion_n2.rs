//! N2 promotion migration gate（Living `16`）。
//!
//! 覆盖：Integer↔Rational · Exact↔Machine · Machine↔Arbitrary · mismatch 诊断。
//! CI 显式运行：`cargo test -p athena-numeric --test numeric_promotion_n2`

use athena_numeric::{
    BigFloat, DefaultNumericCompare, DefaultPromotion, Integer, NumericCompare, NumericComparison, NumericDomain,
    NumericValue, PrecisionKind, Promotion, PromotionPolicy, Rational, Real,
};

#[test]
fn promotion_integer_to_rational() {
    let a = NumericValue::integer(Integer::from_i64(5));
    let promoted = DefaultPromotion::promote(a, &NumericDomain::Rational, &PromotionPolicy::default()).unwrap();
    assert_eq!(promoted.domain(), NumericDomain::Rational);
    match promoted {
        NumericValue::Rational(r) => {
            assert_eq!(r.numerator().to_decimal_string(), "5");
            assert_eq!(r.denominator().to_decimal_string(), "1");
        }
        _ => panic!("expected rational"),
    }
}

#[test]
fn promotion_rational_to_integer_exact() {
    let r = NumericValue::rational(Rational::new(Integer::from_i64(6), Integer::from_i64(2)));
    let promoted = DefaultPromotion::promote(r, &NumericDomain::Integer, &PromotionPolicy::default()).unwrap();
    match promoted {
        NumericValue::Integer(n) => assert_eq!(n.to_decimal_string(), "3"),
        _ => panic!("expected integer"),
    }
}

#[test]
fn promotion_rational_to_integer_forbidden() {
    let r = NumericValue::rational(Rational::new(Integer::from_i64(1), Integer::from_i64(3)));
    let err = DefaultPromotion::promote(r, &NumericDomain::Integer, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn promotion_common_domain_integer_rational() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::rational(Rational::new(Integer::from_i64(2), Integer::from_i64(2)));
    let domain = DefaultPromotion::common_domain(&a, &b, &PromotionPolicy::default()).unwrap();
    assert_eq!(domain, NumericDomain::Rational);
}

#[test]
fn promotion_common_domain_exact_real_requires_policy() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::machine_real(1.0);
    let err = DefaultPromotion::common_domain(&a, &b, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DOMAIN_MISMATCH");

    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let domain = DefaultPromotion::common_domain(&a, &b, &policy).unwrap();
    assert_eq!(domain, NumericDomain::Real);
}

#[test]
fn promotion_exact_to_machine_requires_policy() {
    let a = NumericValue::integer(Integer::from_i64(7));
    let err = DefaultPromotion::promote(a.clone(), &NumericDomain::Real, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");

    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let m = DefaultPromotion::promote(a, &NumericDomain::Real, &policy).unwrap();
    assert_eq!(m.domain(), NumericDomain::Real);
    assert_eq!(m.precision().kind, PrecisionKind::Machine);
}

#[test]
fn promotion_exact_rational_to_machine_exact() {
    let r = NumericValue::rational(Rational::new(Integer::from_i64(1), Integer::from_i64(2)));
    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let m = DefaultPromotion::promote(r, &NumericDomain::Real, &policy).unwrap();
    assert_eq!(m.as_machine_f64(), Some(0.5));
}

#[test]
fn promotion_rejects_non_exact_integer_to_machine() {
    let n = NumericValue::integer(Integer::from_i64(9_007_199_254_740_993));
    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let err = DefaultPromotion::promote(n, &NumericDomain::Real, &policy).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_PRECISION_LOSS");
}

#[test]
fn promotion_rejects_non_exact_rational_to_machine() {
    let r = NumericValue::rational(Rational::new(Integer::from_i64(1), Integer::from_i64(3)));
    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let err = DefaultPromotion::promote(r, &NumericDomain::Real, &policy).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_PRECISION_LOSS");
}

#[test]
fn promotion_real_to_exact_forbidden() {
    let m = NumericValue::machine_real(1.0);
    let err = DefaultPromotion::promote(m.clone(), &NumericDomain::Integer, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
    let err = DefaultPromotion::promote(m, &NumericDomain::Rational, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn promotion_machine_to_arbitrary_imports_bigfloat() {
    let m = NumericValue::machine_real(1.5);
    let promoted = DefaultPromotion::promote_real_precision(m, PrecisionKind::Arbitrary, &PromotionPolicy::default())
        .expect("promote");
    assert_eq!(promoted.precision().kind, PrecisionKind::Arbitrary);
    assert_eq!(promoted.as_real().and_then(|r| r.as_big_float()).unwrap().to_f64_exact(), Some(1.5));
}

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
fn promotion_domain_mismatch() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::machine_real(1.0);
    let err = DefaultPromotion::common_domain(&a, &b, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DOMAIN_MISMATCH");
}

#[test]
fn compare_integer_and_rational_via_promotion() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::rational(Rational::new(Integer::from_i64(2), Integer::from_i64(2)));
    assert_eq!(DefaultNumericCompare::compare(&a, &b, &Default::default()).unwrap(), NumericComparison::ExactEqual);
    let c = NumericValue::rational(Rational::new(Integer::from_i64(3), Integer::from_i64(2)));
    assert_eq!(DefaultNumericCompare::compare(&a, &c, &Default::default()).unwrap(), NumericComparison::Unequal);
}
