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

#[test]
fn roundtrip_rational_poly_via_large_prime_image() {
    use athena_engine::domains::polynomial::reconstruct_polynomial_from_modular_image;
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    // Large enough for default Wang bound on small rationals.
    let p = rings.intern_over_prime_field(Integer::from_i64(97), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(2, 3).unwrap(), vec![1]).unwrap();
    b.push_term(Number::rational_i64(-1, 2).unwrap(), vec![0]).unwrap();
    let poly = b.build(&rings).unwrap();
    let image = map_polynomial_mod_prime(&poly, p, &rings).unwrap();
    assert!(!image.vanished);
    let rebuilt = reconstruct_polynomial_from_modular_image(&image.image, &image.modulus, q, &rings).unwrap();
    assert_eq!(rebuilt, poly);
}

#[test]
fn modular_groebner_image_then_reconstruct_univariate() {
    use athena_engine::domains::polynomial::{GroebnerLimits, compute_groebner_basis, reconstruct_polynomial_from_modular_image};
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p = rings.intern_over_prime_field(Integer::from_i64(97), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(1, 1).unwrap(), vec![1]).unwrap();
    b.push_term(Number::rational_i64(-1, 2).unwrap(), vec![0]).unwrap(); // x - 1/2
    let poly = b.build(&rings).unwrap();
    let image = map_polynomial_mod_prime(&poly, p, &rings).unwrap();
    let gb = compute_groebner_basis(vec![image.image.clone()], &rings, GroebnerLimits::default()).unwrap();
    let verified = gb.as_verified().expect("verified over Fp");
    assert_eq!(verified.basis().len(), 1);
    let rebuilt = reconstruct_polynomial_from_modular_image(&verified.basis()[0], &image.modulus, q, &rings).unwrap();
    // Monic image reconstructs to a scalar multiple of the input over Q; compare after clearing content via equality of roots:
    // rebuilt should be associate to x - 1/2. With Wang reconstruction of monic Fp basis, expect x + c.
    assert!(!rebuilt.is_zero());
    assert_eq!(rebuilt.terms().len(), 2);
}
