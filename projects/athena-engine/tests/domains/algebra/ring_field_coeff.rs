//! `RingTable` ↔ `FieldTable` 系数域统一。

use athena_engine::{
    domains::{
        algebra::{AlgebraParentId, CoefficientParent},
        polynomial::{CoefficientDomain, MonomialOrder, RingTable, add_polynomial, mul_polynomial},
    },
    runtime::values::numeric_clone::clone_number,
};
use athena_numeric::{Integer, Number};
use athena_types::{FieldId, SymbolId};

#[test]
fn intern_over_prime_field_paths_share_field_id() {
    let mut rings = RingTable::new();
    let via_recommended = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let field = rings.field_table_mut().prime_field(Integer::from_i64(5)).unwrap();
    let via_finite = rings.intern_over_field(field, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    assert_eq!(via_recommended, via_finite);

    let desc = rings.get(via_recommended).unwrap();
    let CoefficientParent::Field(field) = desc.coefficients
    else {
        panic!("expected Field parent");
    };
    assert_eq!(rings.coefficient_parent(via_recommended), Some(CoefficientParent::Field(field)));
    assert_eq!(rings.coefficient_parent(via_recommended).and_then(|p| p.as_algebra_parent()), Some(AlgebraParentId::Field(field)));
}

#[test]
fn unregistered_finite_field_rejected() {
    let mut rings = RingTable::new();
    let err = rings.intern(CoefficientDomain::FiniteField { field: FieldId(99) }, vec![SymbolId(0)], MonomialOrder::Lex).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_UNSUPPORTED_OPERATION");
}

#[test]
fn normalized_finite_field_mul_still_works() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b1 = athena_engine::domains::polynomial::PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(3), vec![0]).unwrap();
    b1.push_term(Number::small_int(1), vec![1]).unwrap();
    let p1 = b1.build(&rings).unwrap();
    let mut b2 = athena_engine::domains::polynomial::PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(4), vec![0]).unwrap();
    let p2 = b2.build(&rings).unwrap();
    let sum = add_polynomial(p1, p2, &rings).unwrap();
    let mut b3 = athena_engine::domains::polynomial::PolynomialBuilder::new(ring);
    b3.push_term(Number::small_int(2), vec![0]).unwrap();
    let p3 = b3.build(&rings).unwrap();
    let prod = mul_polynomial(sum, p3, &rings).unwrap();
    let c0 = prod.terms().iter().find(|t| t.exponents() == [0]).map(|t| clone_number(t.coefficient())).unwrap();
    assert_eq!(c0, Number::small_int(4));
}
