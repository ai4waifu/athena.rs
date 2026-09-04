//! 多项式 canonical 构造：Builder · merge · sort · hash。

use athena_engine::{
    domains::polynomial::{
        CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest, PolynomialResult, RingTable, canonicalize_polynomial,
        execute_polynomial_with_rings, polynomial_canonical_hash,
    },
    runtime::Session,
};
use athena_numeric::Number;
use athena_types::SymbolId;

fn xy_integer_ring(order: MonomialOrder) -> (RingTable, athena_engine::domains::polynomial::RingId) {
    let mut rings = RingTable::new();
    let id = rings.intern(CoefficientDomain::Integer, vec![SymbolId(1), SymbolId(2)], order).expect("valid ring");
    (rings, id)
}

#[test]
fn builder_merges_duplicate_monomials() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::Lex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1, 0]).unwrap();
    b.push_term(Number::small_int(2), vec![1, 0]).unwrap();
    let p = b.build(&rings).unwrap();
    assert_eq!(p.terms().len(), 1);
    assert_eq!(p.terms()[0].coefficient().to_render_string(), "3");
}

#[test]
fn builder_drops_zero_coefficients() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::Lex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(0), vec![0, 1]).unwrap();
    b.push_term(Number::small_int(5), vec![0, 0]).unwrap();
    let p = b.build(&rings).unwrap();
    assert_eq!(p.terms().len(), 1);
    assert_eq!(p.terms()[0].exponents(), vec![0, 0]);
}

#[test]
fn builder_sorts_lex_descending() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::Lex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![0, 1]).unwrap();
    b.push_term(Number::small_int(1), vec![1, 0]).unwrap();
    let p = b.build(&rings).unwrap();
    assert_eq!(p.terms().len(), 2);
    assert_eq!(p.terms()[0].exponents(), vec![1, 0]);
    assert_eq!(p.terms()[1].exponents(), vec![0, 1]);
}

#[test]
fn canonicalize_rejects_exponent_length_mismatch() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::Lex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    let err = b.build(&rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_VARIABLE_MISMATCH");
}

#[test]
fn canonicalize_rejects_non_integer_in_integer_ring() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::Lex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::rational_i64(1, 2).unwrap(), vec![0, 0]).unwrap();
    let err = b.build(&rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_DOMAIN_MISMATCH");
}

#[test]
fn canonical_hash_stable_after_canonicalize() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::GrLex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(2), vec![1, 0]).unwrap();
    b.push_term(Number::small_int(3), vec![1, 0]).unwrap();
    b.push_term(Number::small_int(1), vec![0, 1]).unwrap();
    let raw = b.build(&rings).unwrap();
    let again = canonicalize_polynomial(raw.clone(), &rings).unwrap();
    let h1 = polynomial_canonical_hash(&raw, &rings).unwrap();
    let h2 = polynomial_canonical_hash(&again, &rings).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn session_normalize_via_execute_polynomial() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    let raw = b.build(&session.rings).unwrap();
    let out = session.execute_polynomial(PolynomialRequest::Normalize { polynomial: raw });
    match out {
        PolynomialResult::Exact { value } => {
            let poly = match value {
                athena_engine::domains::polynomial::PolynomialDomainValue::Polynomial(v) => v.inner,
                _ => panic!("expected polynomial value"),
            };
            assert_eq!(poly.terms().len(), 1);
            assert_eq!(poly.terms()[0].coefficient().to_render_string(), "2");
        }
        other => panic!("expected Exact, got {other:?}"),
    }
}

#[test]
fn execute_polynomial_with_rings_normalize() {
    let (rings, ring) = xy_integer_ring(MonomialOrder::Lex);
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(4), vec![0, 0]).unwrap();
    let raw = b.build(&rings).unwrap();
    let out = execute_polynomial_with_rings(PolynomialRequest::Normalize { polynomial: raw }, &rings);
    assert!(matches!(out, PolynomialResult::Exact { .. }));
}
