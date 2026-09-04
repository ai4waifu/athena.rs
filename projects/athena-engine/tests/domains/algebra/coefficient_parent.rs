//! `RingDescriptor.coefficients` 为 `CoefficientParent`，无 `PrimeField`。

use athena_engine::domains::{
    algebra::CoefficientParent,
    polynomial::{CoefficientDomain, MonomialOrder, RingTable},
};
use athena_numeric::{Integer, Modulus};
use athena_types::SymbolId;

#[test]
fn ring_descriptor_coefficients_is_coefficient_parent() {
    let mut rings = RingTable::new();
    let z = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let desc = rings.get(z).unwrap();
    let CoefficientParent::Ring(coeff_ring) = desc.coefficients
    else {
        panic!("expected Ring parent for Z");
    };
    assert_eq!(coeff_ring, desc.coefficient_ring);
    assert!(matches!(rings.coefficient_rings().get(coeff_ring).unwrap().domain, CoefficientDomain::Integer));

    let field = rings.field_table_mut().prime_field(Integer::from_i64(11)).unwrap();
    let fp = rings.intern_over_field(field, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    assert_eq!(rings.get(fp).unwrap().coefficients, CoefficientParent::Field(field));
}

#[test]
fn modular_integer_parent_is_coefficient_ring() {
    let mut rings = RingTable::new();
    let modulus = Modulus::new(Integer::from_i64(8)).unwrap();
    let ring = rings.intern(CoefficientDomain::ModularInteger { modulus }, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let desc = rings.get(ring).unwrap();
    assert!(matches!(desc.coefficients, CoefficientParent::Ring(id) if id == desc.coefficient_ring));
}
