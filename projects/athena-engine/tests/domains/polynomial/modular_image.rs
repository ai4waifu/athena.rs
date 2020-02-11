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
fn crt_combine_two_primes_then_reconstruct() {
    use athena_engine::domains::polynomial::crt_combine_and_reconstruct;
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p7 = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(2, 3).unwrap(), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let poly = b.build(&rings).unwrap();
    let i5 = map_polynomial_mod_prime(&poly, p5, &rings).unwrap();
    let i7 = map_polynomial_mod_prime(&poly, p7, &rings).unwrap();
    assert!(!i5.vanished && !i7.vanished);
    let rebuilt = crt_combine_and_reconstruct(&[i5, i7], z, q, &rings).unwrap();
    assert_eq!(rebuilt, poly);
}

#[test]
fn smoke_groebner_over_fp_then_reconstruct() {
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
    // Monic Fp basis reconstructs to a non-zero bivariate-shaped univariate with two terms.
    assert!(!rebuilt.is_zero());
    assert_eq!(rebuilt.terms().len(), 2);
}

#[test]
fn modular_image_via_polynomial_request() {
    use athena_engine::domains::polynomial::{PolynomialObjectStore, PolynomialRequest, execute_polynomial_with_rings};
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let f5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(z);
    b.push_term(Number::small_int(12), vec![1]).unwrap();
    let poly = b.build(&rings).unwrap();
    let mut store = PolynomialObjectStore::new();
    let pref = store.intern(poly, &rings);
    let result = execute_polynomial_with_rings(PolynomialRequest::ModularImage { polynomial: pref, image_ring: f5 }, &rings, &store);
    match result {
        athena_engine::domains::polynomial::PolynomialResult::Exact {
            value: athena_engine::domains::polynomial::PolynomialDomainValue::ModularImage(image),
        } => {
            assert!(!image.vanished);
            assert_eq!(image.image.terms().len(), 1);
            assert_eq!(image.image.terms()[0].coefficient().to_render_string(), "2");
        }
        other => panic!("expected ModularImage exact, got {other:?}"),
    }
}

#[test]
fn reconstruct_modular_via_polynomial_request() {
    use athena_engine::domains::polynomial::{PolynomialObjectStore, PolynomialRequest, execute_polynomial_with_rings};
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p = rings.intern_over_prime_field(Integer::from_i64(97), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(2, 3).unwrap(), vec![1]).unwrap();
    b.push_term(Number::rational_i64(-1, 2).unwrap(), vec![0]).unwrap();
    let poly = b.build(&rings).unwrap();
    let image = map_polynomial_mod_prime(&poly, p, &rings).unwrap();
    let mut store = PolynomialObjectStore::new();
    let image_ref = store.intern(image.image.owning_copy(), &rings);
    let result = execute_polynomial_with_rings(
        PolynomialRequest::ReconstructModular { image: image_ref, target_ring: q },
        &rings,
        &store,
    );
    match result {
        athena_engine::domains::polynomial::PolynomialResult::Exact {
            value: athena_engine::domains::polynomial::PolynomialDomainValue::Polynomial(v),
        } => assert_eq!(v.inner, poly),
        other => panic!("expected reconstructed polynomial, got {other:?}"),
    }
}

#[test]
fn crt_combine_modular_via_polynomial_request() {
    use athena_engine::domains::polynomial::{PolynomialObjectStore, PolynomialRequest, execute_polynomial_with_rings};
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p7 = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(2, 3).unwrap(), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let poly = b.build(&rings).unwrap();
    let i5 = map_polynomial_mod_prime(&poly, p5, &rings).unwrap();
    let i7 = map_polynomial_mod_prime(&poly, p7, &rings).unwrap();
    let mut store = PolynomialObjectStore::new();
    let r5 = store.intern(i5.image.owning_copy(), &rings);
    let r7 = store.intern(i7.image.owning_copy(), &rings);
    let result = execute_polynomial_with_rings(
        PolynomialRequest::CrtCombineModular { images: vec![r5, r7], integer_ring: z, target_ring: q },
        &rings,
        &store,
    );
    match result {
        athena_engine::domains::polynomial::PolynomialResult::Exact {
            value: athena_engine::domains::polynomial::PolynomialDomainValue::Polynomial(v),
        } => assert_eq!(v.inner, poly),
        other => panic!("expected CRT reconstructed polynomial, got {other:?}"),
    }
}

#[test]
fn reconstruct_single_generator_groebner_via_two_primes() {
    use athena_engine::domains::polynomial::{GroebnerLimits, reconstruct_groebner_basis_via_crt};
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p7 = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(2, 3).unwrap(), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap(); // (2/3)x - 1 → monic GB is x - 3/2
    let poly = b.build(&rings).unwrap();
    let basis = reconstruct_groebner_basis_via_crt(&[poly.clone()], &[p5, p7], z, q, &rings, GroebnerLimits::default()).unwrap();
    assert_eq!(basis.len(), 1);
    // Current Buchberger path does not force monic bases. CRT recovers the modular images of the generator.
    assert_eq!(basis[0], poly);
}

#[test]
fn reconstruct_and_verify_single_generator_groebner_via_crt() {
    use athena_engine::domains::polynomial::{GroebnerLimits, reconstruct_and_verify_groebner_basis_via_crt};
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p5 = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p7 = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::rational_i64(2, 3).unwrap(), vec![1]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let poly = b.build(&rings).unwrap();
    let (basis, report) =
        reconstruct_and_verify_groebner_basis_via_crt(&[poly.clone()], &[p5, p7], z, q, &rings, GroebnerLimits::default()).unwrap();
    assert!(report.all_s_pairs_reduce_to_zero);
    assert_eq!(basis, vec![poly]);
}
