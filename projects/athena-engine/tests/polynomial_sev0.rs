//! SEV-0 数学正确性：环特征 · 素域验证 · 指数溢出 · canonical 排序错误传播。

use athena_engine::{
    CoefficientDomain, CoefficientParent, Integer, MonomialOrder, PolynomialBuilder, RingCharacteristic, RingTable, SymbolId,
    add_polynomial, mul_polynomial,
};
use athena_numeric::{Modulus, Number};

#[test]
fn modular_integer_characteristic_is_modulus() {
    let mut table = RingTable::new();
    let modulus = Modulus::new(Integer::from_i64(6)).unwrap();
    let ring = table.intern(CoefficientDomain::ModularInteger { modulus }, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let desc = table.get(ring).unwrap();
    assert_eq!(desc.characteristic, RingCharacteristic::Positive(Integer::from_i64(6)));
}

#[test]
fn intern_over_prime_field_uses_field_parent() {
    let mut table = RingTable::new();
    let ring = table.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let desc = table.get(ring).unwrap();
    let CoefficientParent::Field(field) = desc.coefficients else {
        panic!("expected Field parent");
    };
    assert_eq!(desc.characteristic, RingCharacteristic::Positive(Integer::from_i64(5)));
    assert!(table.field_table().presentation(field).is_some());
}

#[test]
fn prime_field_characteristic_is_p() {
    let mut table = RingTable::new();
    let ring = table.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let desc = table.get(ring).unwrap();
    assert_eq!(desc.characteristic, RingCharacteristic::Positive(Integer::from_i64(5)));
}

#[test]
fn composite_prime_field_rejected() {
    let mut table = RingTable::new();
    let err = table.intern_over_prime_field(Integer::from_i64(6), vec![SymbolId(0)], MonomialOrder::Lex).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_MODULUS_INVALID");
}

#[test]
fn finite_field_characteristic_from_descriptor() {
    let mut table = RingTable::new();
    let field = table.field_table_mut().prime_field(Integer::from_i64(7)).unwrap();
    let ring = table.intern_over_field(field, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let desc = table.get(ring).unwrap();
    assert_eq!(desc.characteristic, RingCharacteristic::Positive(Integer::from_i64(7)));
    let coeff_ring = desc.coefficient_ring;
    assert!(matches!(
        table.coeff_rings().coefficient_parent(coeff_ring),
        CoefficientParent::Field(f) if f == field
    ));
}

#[test]
fn exponent_overflow_on_mul_rejected() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![u32::MAX]).unwrap();
    let p1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(1), vec![1]).unwrap();
    let x = b2.build(&rings).unwrap();
    let err = mul_polynomial(p1, x, &rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_DEGREE_OVERFLOW");
}

#[test]
fn exponent_overflow_on_add_monomials_ok_when_distinct() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(1), vec![u32::MAX]).unwrap();
    let p1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(1), vec![1]).unwrap();
    let p2 = b2.build(&rings).unwrap();
    let sum = add_polynomial(p1, p2, &rings).unwrap();
    assert_eq!(sum.terms.len(), 2);
}

#[test]
fn canonical_sort_propagates_exponent_length_error() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Integer, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![1, 0]).unwrap();
    b.push_term(Number::small_int(1), vec![0]).unwrap();
    let err = b.build(&rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_VARIABLE_MISMATCH");
}
