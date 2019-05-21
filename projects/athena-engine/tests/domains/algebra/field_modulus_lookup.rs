//! 系数模数经 `FieldPresentation` 查找，有限域无重复 characteristic。

use athena_engine::domains::polynomial::{CoefficientDomain, MonomialOrder, RingTable};
use athena_numeric::{Integer, Number};
use athena_types::{FieldId, SymbolId};

#[test]
fn finite_field_domain_is_field_id_only() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(13), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let coeff_id = rings.get(ring).unwrap().coefficient_ring;
    assert!(matches!(rings.coeff_rings().get(coeff_id).unwrap().domain, CoefficientDomain::FiniteField { field: _ }));
}

#[test]
fn coeff_kernel_modulus_from_presentation_not_domain_payload() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let kernel = rings.coeff_kernel(ring).unwrap();
    let prod = kernel.mul(Number::small_int(3), Number::small_int(5)).unwrap();
    assert_eq!(prod, Number::small_int(1));
}

#[test]
fn unregistered_field_id_rejected_without_characteristic_in_domain() {
    let mut rings = RingTable::new();
    let err =
        rings.intern(CoefficientDomain::FiniteField { field: FieldId(42) }, vec![SymbolId(0)], MonomialOrder::Lex).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_UNSUPPORTED_OPERATION");
}

#[test]
fn same_field_id_reuses_coeff_ring_regardless_of_session_order() {
    let mut rings = RingTable::new();
    let f = rings.field_table_mut().prime_field(Integer::from_i64(5)).unwrap();
    let r1 = rings.intern_over_field(f, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let r2 = rings.intern_over_field(f, vec![SymbolId(1)], MonomialOrder::GrLex).unwrap();
    assert_eq!(rings.get(r1).unwrap().coefficient_ring, rings.get(r2).unwrap().coefficient_ring);
}
