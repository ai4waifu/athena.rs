//! `RingHandle` vs `RingFingerprint` 与 M-Graph witness 合同。

use athena_engine::{
    domains::polynomial::{
        CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialCacheOp, PolynomialRequest, RingTable, cache_key_for_request, fnv1a64,
        polynomial_canonical_hash, polynomial_fingerprint,
    },
    runtime::Session,
};
use athena_numeric::Number;
use athena_types::SymbolId;

#[test]
fn ring_fingerprint_stable_across_sessions() {
    let mut a = RingTable::new();
    let mut b = RingTable::new();
    let ring_a = a.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let ring_b = b.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    assert_eq!(a.get(ring_a).unwrap().ring_fingerprint, b.get(ring_b).unwrap().ring_fingerprint);
}

#[test]
fn cache_key_uses_ring_fingerprint_not_handle() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p = PolynomialBuilder::new(ring).build(&session.rings).unwrap();
    let key = cache_key_for_request(&PolynomialRequest::Normalize { polynomial: p }, &session.rings).unwrap();
    assert_eq!(key.ring_fingerprint, session.rings.ring_fingerprint(ring).unwrap());
    assert_eq!(key.input_fingerprints.len(), 1);
}

#[test]
fn polynomial_fingerprint_stable_after_canonicalize() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Integer, vec![SymbolId(1), SymbolId(2)], MonomialOrder::GrLex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(2), vec![1, 0]).unwrap();
    b.push_term(Number::small_int(3), vec![1, 0]).unwrap();
    b.push_term(Number::small_int(1), vec![0, 1]).unwrap();
    let raw = b.build(&rings).unwrap();
    let fp1 = polynomial_fingerprint(&raw, &rings).unwrap();
    let fp2 = polynomial_fingerprint(&raw, &rings).unwrap();
    assert_eq!(fp1, fp2);
    assert_eq!(fp1.0, polynomial_canonical_hash(&raw, &rings).unwrap());
}

#[test]
fn cache_key_distinguishes_operations_with_fingerprints() {
    let mut session = Session::default();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let p = PolynomialBuilder::new(ring).build(&session.rings).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![0]).unwrap();
    let q = b.build(&session.rings).unwrap();
    let k_add = cache_key_for_request(&PolynomialRequest::Add { lhs: p.clone(), rhs: q.clone() }, &session.rings).unwrap();
    let k_mul = cache_key_for_request(&PolynomialRequest::Mul { lhs: p, rhs: q }, &session.rings).unwrap();
    assert_ne!(k_add.operation, k_mul.operation);
    assert_eq!(k_add.operation, PolynomialCacheOp::Add);
    assert_eq!(k_add.ring_fingerprint, k_mul.ring_fingerprint);
}

#[test]
fn fnv1a64_empty_is_offset_basis() {
    assert_eq!(fnv1a64(&[]), 0xcbf29ce484222325);
}

#[test]
fn ring_fingerprint_independent_of_ring_handle_allocation() {
    let mut a = RingTable::new();
    let mut b = RingTable::new();
    let ring_a = a.intern(CoefficientDomain::Integer, vec![SymbolId(1)], MonomialOrder::Lex).unwrap();
    let ring_b = b.intern(CoefficientDomain::Integer, vec![SymbolId(1)], MonomialOrder::Lex).unwrap();
    assert_eq!(a.get(ring_a).unwrap().ring_fingerprint, b.get(ring_b).unwrap().ring_fingerprint);
}
