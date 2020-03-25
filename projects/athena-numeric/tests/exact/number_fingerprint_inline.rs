//! 自 `src/value/number.rs` 迁出的原内联测试。

use athena_numeric::{
    algebraic::AlgebraicNumber,
    complex::Complex,
    decimal::Decimal,
    domain::NumericDomain,
    finite_field::FiniteFieldValue,
    integer::{Integer, Sign},
    interval::Interval,
    modular::ModularValue,
    p_adic::PAdicValue,
    precision::PrecisionInfo,
    rational::Rational,
    real::Real,
    *,
};
use athena_types::{Diagnostic, DiagnosticCode, Result};

#[test]
fn integer_content_hash_is_limb_stable() {
    let a = Number::Integer(Integer::from_i64(42));
    let b = Number::Integer(Integer::from_i64(42));
    let c = Number::Integer(Integer::from_i64(43));
    assert_eq!(a.fingerprint_content_hash(), b.fingerprint_content_hash());
    assert_ne!(a.fingerprint_content_hash(), c.fingerprint_content_hash());
    assert_eq!(a.fingerprint_domain_tag(), 1);
}
