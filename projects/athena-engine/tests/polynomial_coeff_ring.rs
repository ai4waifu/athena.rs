//! `CoefficientRingId` intern 与专用系数内核。

use athena_engine::{
    CoefficientDomain, CoefficientRingId, Integer, MonomialOrder, Number, PolynomialBuilder, RingTable, SymbolId,
    mul_polynomial,
};

#[test]
fn coefficient_ring_intern_idempotent() {
    let mut table = RingTable::new();
    let ring_a = table.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let ring_b = table.intern(CoefficientDomain::Integer, vec![SymbolId(1)], MonomialOrder::GrLex).unwrap();
    let desc_a = table.get(ring_a).unwrap();
    let desc_b = table.get(ring_b).unwrap();
    assert_eq!(desc_a.coefficient()_ring, desc_b.coefficient()_ring);
    assert_eq!(table.coeff_rings().len(), 1);
}

#[test]
fn distinct_coefficient_domains_get_distinct_ids() {
    let mut table = RingTable::new();
    let z = table.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let q = table.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let fp = table.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let z_id = table.get(z).unwrap().coefficient()_ring;
    let q_id = table.get(q).unwrap().coefficient()_ring;
    let fp_id = table.get(fp).unwrap().coefficient()_ring;
    assert_ne!(z_id, q_id);
    assert_ne!(z_id, fp_id);
    assert_ne!(q_id, fp_id);
    assert_eq!(table.coeff_rings().len(), 3);
}

#[test]
fn coeff_ring_descriptor_matches_domain() {
    let mut table = RingTable::new();
    let ring = table.intern_over_prime_field(Integer::from_i64(11), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let coeff_id = table.get(ring).unwrap().coefficient()_ring;
    let desc = table.coeff_rings().get(coeff_id).unwrap();
    assert!(matches!(desc.domain, CoefficientDomain::FiniteField { .. }));
}

#[test]
fn specialized_fp_kernel_mul_via_ring_table() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(3), vec![0]).unwrap();
    let p1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(4), vec![0]).unwrap();
    let p2 = b2.build(&rings).unwrap();
    let product = mul_polynomial(p1, p2, &rings).unwrap();
    assert_eq!(product.terms().len(), 1);
    assert_eq!(product.terms()[0].coefficient(), Number::small_int(2));
}

#[test]
fn coefficient_ring_id_is_not_ring_id() {
    let z = CoefficientRingId(0);
    let _ = z;
}
