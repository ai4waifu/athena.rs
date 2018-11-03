//! N0–N1 数值塔验收测试（backend · ExactInteger/Rational · serialize · compare 宿主）。

use athena_numeric::{
    DefaultNumericCompare, Integer, NumericBackend, NumericCompare, NumericComparison, NumericDomain, NumericValue,
    NumericValueWire, PureRustBackend, Rational, Sign,
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
    assert!(matches!(back, NumericValue::Integer(_)));
    assert_eq!(DefaultNumericCompare::compare(&v, &back, &Default::default()).unwrap(), NumericComparison::ExactEqual);

    let r = NumericValue::rational(Rational::new(Integer::from_i64(3), Integer::from_i64(6)));
    let wire = NumericValueWire::encode(&r).unwrap();
    let back = wire.decode().unwrap();
    assert_eq!(DefaultNumericCompare::compare(&r, &back, &Default::default()).unwrap(), NumericComparison::ExactEqual);
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
    use athena_numeric::PrecisionInfo;
    let err = NumericValue::try_new(
        NumericDomain::Integer,
        NumericValue::machine_real(1.0),
        PrecisionInfo::exact(),
    )
    .unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DOMAIN_MISMATCH");
}
