//! 多项式模同态 ℤ/ℚ → 𝔽_p。

use athena_engine::domains::polynomial::{
    CoefficientDomain, MonomialOrder, PolynomialBuilder, RingTable, map_polynomial_mod_prime,
};
use athena_numeric::{Integer, Number};
use athena_types::SymbolId;

#[test]
fn integer_coefficients_reduce_mod_5() {
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let f5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(z);
    b.push_term(Number::small_int(12), vec![1]).unwrap(); // 12 ≡ 2
    b.push_term(Number::small_int(5), vec![0]).unwrap(); // 5 ≡ 0 → drop
    let poly = b.build(&rings).unwrap();
    let image = map_polynomial_mod_prime(&poly, f5, &rings).unwrap();
    assert!(!image.vanished);
    assert_eq!(image.image.terms().len(), 1);
    assert_eq!(image.image.terms()[0].coefficient().to_render_string(), "2");
    assert_eq!(image.image.terms()[0].exponents(), vec![1]);
}

#[test]
fn rational_coefficient_needs_invertible_denominator() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let f5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(1, 2).unwrap(), vec![0]).unwrap(); // 2^{-1} ≡ 3 (mod 5) → 3
    let poly = b.build(&rings).unwrap();
    let image = map_polynomial_mod_prime(&poly, f5, &rings).unwrap();
    assert_eq!(image.image.terms().len(), 1);
    assert_eq!(image.image.terms()[0].coefficient().to_render_string(), "3");
}

#[test]
fn rational_bad_denominator_is_rejected() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let f5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(1, 5).unwrap(), vec![0]).unwrap();
    let poly = b.build(&rings).unwrap();
    let err = map_polynomial_mod_prime(&poly, f5, &rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_MODULAR_INVERSE_MISSING");
}

#[test]
fn multiple_of_prime_vanishes() {
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let f5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(z);
    b.push_term(Number::small_int(10), vec![2]).unwrap();
    let poly = b.build(&rings).unwrap();
    let image = map_polynomial_mod_prime(&poly, f5, &rings).unwrap();
    assert!(image.vanished);
    assert!(image.image.is_zero());
}
