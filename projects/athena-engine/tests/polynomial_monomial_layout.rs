//! MonomialLayout + compiled monomial order（环 intern 时编译）。

use athena_engine::{CoefficientDomain, MonomialLayout, MonomialOrder, RingTable, SymbolId};

#[test]
fn ring_descriptor_carries_compiled_layout() {
    let mut table = RingTable::new();
    let ring = table.intern(CoefficientDomain::Integer, vec![SymbolId(0), SymbolId(1)], MonomialOrder::GrLex).unwrap();
    let desc = table.get(ring).unwrap();
    assert_eq!(desc.monomial_layout.variable_count(), 2);
    assert_eq!(desc.monomial_layout.bits_per_exponent(), 32);
}

#[test]
fn compiled_grlex_matches_declarative_order() {
    let layout = MonomialLayout::compile(&MonomialOrder::GrLex, 2).unwrap();
    let a = vec![2, 0];
    let b = vec![1, 1];
    assert!(layout.cmp_exponents(&a, &b) == std::cmp::Ordering::Greater);
    assert!(layout.cmp_exponents_desc(&a, &b) == std::cmp::Ordering::Less);
}

#[test]
fn compiled_elimination_order_smoke() {
    let order = MonomialOrder::Elimination { eliminate: 1, rest: Box::new(MonomialOrder::Lex) };
    let layout = MonomialLayout::compile(&order, 2).unwrap();
    let front = vec![1, 0];
    let back = vec![0, 1];
    assert!(layout.cmp_exponents(&front, &back) == std::cmp::Ordering::Greater);
}

#[test]
fn canonical_sort_uses_layout_not_runtime_enum() {
    let mut table = RingTable::new();
    let ring = table.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::GrLex).unwrap();
    let mut b = athena_engine::PolynomialBuilder::new(ring);
    b.push_term(athena_engine::Number::small_int(1), vec![1, 0]).unwrap();
    b.push_term(athena_engine::Number::small_int(1), vec![0, 1]).unwrap();
    b.push_term(athena_engine::Number::small_int(1), vec![2, 0]).unwrap();
    let poly = b.build(&table).unwrap();
    assert_eq!(poly.terms.len(), 3);
    assert_eq!(poly.terms[0].exponents, vec![2, 0]);
    assert_eq!(poly.terms[1].exponents, vec![1, 0]);
    assert_eq!(poly.terms[2].exponents, vec![0, 1]);
}

#[test]
fn distinct_orders_distinct_layouts() {
    let lex = MonomialLayout::compile(&MonomialOrder::Lex, 2).unwrap();
    let weighted = MonomialLayout::compile(&MonomialOrder::Weighted { weights: vec![1, 10] }, 2).unwrap();
    let e1 = vec![1, 0];
    let e2 = vec![0, 1];
    assert_ne!(lex.cmp_exponents(&e1, &e2), weighted.cmp_exponents(&e1, &e2));
}
