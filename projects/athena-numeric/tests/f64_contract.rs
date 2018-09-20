//! Rational / promotion 精确转换合同测试。

use athena_numeric::{DefaultPromotion, Integer, NumericDomain, NumericValue, Promotion, PromotionPolicy, Rational};

#[test]
fn rational_one_third_not_exact_f64() {
    let r = Rational::new(Integer::from_i64(1), Integer::from_i64(3));
    assert_eq!(r.try_to_f64_exact(), None);
    assert!(r.to_f64_approximate().is_some());
}

#[test]
fn rational_one_half_exact_f64() {
    let r = Rational::new(Integer::from_i64(1), Integer::from_i64(2));
    assert_eq!(r.try_to_f64_exact(), Some(0.5));
}

#[test]
fn promotion_rejects_non_exact_integer_to_machine() {
    let n = NumericValue::integer(Integer::from_i64(9_007_199_254_740_993));
    let policy = PromotionPolicy { allow_exact_to_machine: true, allow_arbitrary_to_machine: false };
    let err = DefaultPromotion::promote(n, &NumericDomain::Real, &policy).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_PRECISION_LOSS");
}
