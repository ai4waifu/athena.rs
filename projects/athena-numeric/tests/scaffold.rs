//! N0 数值塔骨架冒烟测试。

use athena_numeric::{
    DefaultNumericCompare, DefaultPromotion, Integer, NumericCompare, NumericComparison, NumericValue, Promotion,
    PromotionPolicy, PureRustBackend, NumericBackend, Rational,
};
use athena_types::NumericKind;

#[test]
fn integer_rational_constructors() {
    let n = Integer::from_i64(42);
    assert_eq!(n.to_decimal_string(), "42");
    let r = Rational::new(Integer::from_i64(1), Integer::from_i64(2));
    assert_eq!(r.numerator().to_decimal_string(), "1");
    assert_eq!(r.denominator().to_decimal_string(), "2");
}

#[test]
fn numeric_value_integer_roundtrip_domain() {
    let v = NumericValue::integer(Integer::from_i64(7));
    assert!(matches!(v.value, athena_numeric::NumericRepr::Integer(_)));
}

#[test]
fn default_promotion_same_domain() {
    let a = NumericValue::integer(Integer::from_i64(1));
    let b = NumericValue::integer(Integer::from_i64(2));
    let domain = DefaultPromotion::common_domain(&a, &b, &PromotionPolicy::default()).expect("same domain");
    assert_eq!(domain, athena_numeric::NumericDomain::Integer);
}

#[test]
fn compare_integers() {
    let a = NumericValue::integer(Integer::from_i64(3));
    let b = NumericValue::integer(Integer::from_i64(3));
    let c = NumericValue::integer(Integer::from_i64(4));
    assert_eq!(
        DefaultNumericCompare::compare(&a, &b, &Default::default()).unwrap(),
        NumericComparison::ExactEqual
    );
    assert_eq!(
        DefaultNumericCompare::compare(&a, &c, &Default::default()).unwrap(),
        NumericComparison::Unequal
    );
}

#[test]
fn pure_rust_backend_is_wasm_safe() {
    assert!(PureRustBackend.wasm_safe());
    assert_eq!(PureRustBackend.name(), "pure-rust");
}

#[test]
fn numeric_kind_covers_tower() {
    let kinds = [
        NumericKind::Integer,
        NumericKind::Rational,
        NumericKind::Real,
        NumericKind::Complex,
        NumericKind::Interval,
        NumericKind::Algebraic,
        NumericKind::FiniteField,
        NumericKind::Modular,
        NumericKind::PAdic,
    ];
    assert_eq!(kinds.len(), 9);
}
