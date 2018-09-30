//! N0–N2 数值塔验收测试。

use athena_numeric::{
    DefaultNumericCompare, DefaultPromotion, Integer, NumericBackend, NumericCompare, NumericComparison, NumericDomain,
    NumericRepr, NumericValue, NumericValueWire, PrecisionKind, Promotion, PromotionPolicy, PureRustBackend, Rational, Sign,
};

#[test]
fn integer_rational_constructors_and_sign() {
    let n = Integer::from_i64(-42);
    assert_eq!(n.to_decimal_string(), "-42");
    assert_eq!(n.sign(), Sign::Negative);
    let r = Rational::try_new(Integer::from_i64(2), Integer::from_i64(4)).unwrap();
    assert_eq!(r.numerator().to_decimal_string(), "1");
    assert_eq!(r.denominator().to_decimal_string(), "2");
    assert_eq!(r.sign(), Sign::Positive);
}

#[test]
fn integer_gcd_and_normalize() {
    let g = Integer::from_i64(48).gcd(&Integer::from_i64(18));
    assert_eq!(g.to_decimal_string(), "6");
    let r = Rational::new(Integer::from_i64(-2), Integer::from_i64(-4)).normalize();
    assert_eq!(r.numerator().to_decimal_string(), "1");
    assert_eq!(r.denominator().to_decimal_string(), "2");
}

#[test]
fn rational_zero_denom_is_error() {
    let err = Rational::try_new(Integer::from_i64(1), Integer::zero()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_DIVIDE_BY_ZERO");
}

#[test]
fn serialize_integer_rational_roundtrip() {
    let v = NumericValue::integer(Integer::from_i64(99));
    let wire = NumericValueWire::encode(&v).unwrap();
    let back = wire.decode().unwrap();
    assert!(matches!(back.repr(), NumericRepr::Integer(_)));
    assert_eq!(DefaultNumericCompare::compare(&v, &back, &Default::default()).unwrap(), NumericComparison::ExactEqual);

    let r = NumericValue::rational(Rational::new(Integer::from_i64(3), Integer::from_i64(6)));
    let wire = NumericValueWire::encode(&r).unwrap();
    let back = wire.decode().unwrap();
    assert_eq!(DefaultNumericCompare::compare(&r, &back, &Default::default()).unwrap(), NumericComparison::ExactEqual);
}

#[test]
fn compare_integer_and_rational() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::rational(Rational::new(Integer::from_i64(2), Integer::from_i64(2)));
    assert_eq!(DefaultNumericCompare::compare(&a, &b, &Default::default()).unwrap(), NumericComparison::ExactEqual);
    let c = NumericValue::rational(Rational::new(Integer::from_i64(3), Integer::from_i64(2)));
    assert_eq!(DefaultNumericCompare::compare(&a, &c, &Default::default()).unwrap(), NumericComparison::Unequal);
}

#[test]
fn promotion_integer_to_rational() {
    let a = NumericValue::integer(Integer::from_i64(5));
    let domain = NumericDomain::Rational;
    let promoted = DefaultPromotion::promote(a, &domain, &PromotionPolicy::default()).unwrap();
    assert_eq!(*promoted.domain(), NumericDomain::Rational);
    match promoted.repr() {
        NumericRepr::Rational(r) => {
            assert_eq!(r.numerator().to_decimal_string(), "5");
            assert_eq!(r.denominator().to_decimal_string(), "1");
        }
        _ => panic!("expected rational"),
    }
}

#[test]
fn promotion_exact_to_machine_requires_policy() {
    let a = NumericValue::integer(Integer::from_i64(7));
    let err = DefaultPromotion::promote(a.clone(), &NumericDomain::Real, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");

    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let m = DefaultPromotion::promote(a, &NumericDomain::Real, &policy).unwrap();
    assert_eq!(*m.domain(), NumericDomain::Real);
    assert_eq!(m.precision().kind, PrecisionKind::Machine);
}

#[test]
fn promotion_machine_to_arbitrary_rejected_until_bigfloat() {
    let m = NumericValue::machine_real(1.5);
    let err = DefaultPromotion::promote_real_precision(m, PrecisionKind::Arbitrary, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn promotion_domain_mismatch() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::machine_real(1.0);
    let err = DefaultPromotion::common_domain(&a, &b, &PromotionPolicy::default()).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DOMAIN_MISMATCH");
}

#[test]
fn pure_rust_backend_is_wasm_safe() {
    use athena_numeric::NumericCapability;
    let b = PureRustBackend;
    assert!(b.wasm_safe());
    assert_eq!(b.name(), "pure-rust");
    assert!(b.has_capability(NumericCapability::ExactInteger));
    assert!(b.supports_domain(&NumericDomain::Integer));
}

#[test]
fn try_new_rejects_domain_repr_mismatch() {
    use athena_numeric::{NumericProvenance, PrecisionInfo};
    let err = NumericValue::try_new(
        NumericDomain::Integer,
        NumericRepr::Real(athena_numeric::Real::machine(1.0)),
        PrecisionInfo::exact(),
        NumericProvenance::default(),
    )
    .unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DOMAIN_MISMATCH");
}
