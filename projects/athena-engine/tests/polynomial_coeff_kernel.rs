//! Phase：专用系数内核 Z / Q / FpWord / FpBig。

use athena_engine::{
    CoefficientDomain, Integer, MonomialOrder, Number, PolynomialBuilder, RingTable, SymbolId, add_polynomial, mul_polynomial,
};

#[test]
fn integer_and_rational_select_dedicated_kernels() {
    let mut table = RingTable::new();
    let z = table.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let q = table.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    assert_eq!(table.coeff_kernel(z).unwrap().kind_tag(), "Z");
    assert_eq!(table.coeff_kernel(q).unwrap().kind_tag(), "Q");
}

#[test]
fn small_prime_field_selects_fp_word_kernel() {
    let mut table = RingTable::new();
    let ring = table.intern_over_prime_field(Integer::from_i64(17), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    assert_eq!(table.coeff_kernel(ring).unwrap().kind_tag(), "FpWord");
}

#[test]
fn fp_word_kernel_mul_and_inv() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(7), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(3), vec![1]).unwrap();
    let p = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(5), vec![0]).unwrap();
    let c = b2.build(&rings).unwrap();
    // 3x * 5 = 15x ≡ x (mod 7)
    let product = mul_polynomial(p, c, &rings).unwrap();
    assert_eq!(product.terms().len(), 1);
    assert_eq!(product.terms()[0].coefficient(), &Number::small_int(1));
    assert_eq!(product.terms()[0].exponents(), vec![1]);
}

#[test]
fn fp_word_kernel_neg_wrap() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(2), vec![0]).unwrap();
    let p = b.build(&rings).unwrap();
    let zero = PolynomialBuilder::new(ring).build(&rings).unwrap();
    // 0 - 2 ≡ 3 (mod 5)
    let neg = athena_engine::sub_polynomial(zero, p, &rings).unwrap();
    assert_eq!(neg.terms()[0].coefficient(), &Number::small_int(3));
}

#[test]
fn fp_word_kernel_add_reduces() {
    let mut rings = RingTable::new();
    let ring = rings.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(ring);
    b1.push_term(Number::small_int(3), vec![0]).unwrap();
    let a = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(ring);
    b2.push_term(Number::small_int(4), vec![0]).unwrap();
    let b = b2.build(&rings).unwrap();
    let sum = add_polynomial(a, b, &rings).unwrap();
    assert_eq!(sum.terms()[0].coefficient(), &Number::small_int(2));
}

#[test]
fn same_small_prime_reuses_word_kernel_entry() {
    let mut table = RingTable::new();
    let a = table.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let b = table.intern_over_prime_field(Integer::from_i64(5), vec![SymbolId(1)], MonomialOrder::GrLex).unwrap();
    assert_eq!(table.get(a).unwrap().coefficient_ring, table.get(b).unwrap().coefficient_ring);
    assert_eq!(table.coeff_kernel(a).unwrap().kind_tag(), "FpWord");
}
