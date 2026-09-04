//! `MonomialLayout` + 已编译单项式序（环 intern 时编译）。

use athena_engine::domains::polynomial::{CoefficientDomain, MonomialLayout, MonomialOrder, RingTable};
use athena_types::SymbolId;

#[test]
fn ring_descriptor_carries_compiled_layout() {
    let mut table = RingTable::new();
    let ring = table.intern(CoefficientDomain::Integer, vec![SymbolId(0), SymbolId(1)], MonomialOrder::GrLex).unwrap();
    let desc = table.get(ring).unwrap();
    assert_eq!(desc.monomial_layout.variable_count(), 2);
    assert_eq!(desc.monomial_layout.bits_per_exponent(), 16);
    assert_eq!(desc.monomial_layout.packed_words_per_monomial(), 1);
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
    let mut b = athena_engine::domains::polynomial::PolynomialBuilder::new(ring);
    b.push_term(athena_numeric::Number::small_int(1), vec![1, 0]).unwrap();
    b.push_term(athena_numeric::Number::small_int(1), vec![0, 1]).unwrap();
    b.push_term(athena_numeric::Number::small_int(1), vec![2, 0]).unwrap();
    let poly = b.build(&table).unwrap();
    assert_eq!(poly.terms().len(), 3);
    assert_eq!(poly.terms()[0].exponents(), vec![2, 0]);
    assert_eq!(poly.terms()[1].exponents(), vec![1, 0]);
    assert_eq!(poly.terms()[2].exponents(), vec![0, 1]);
}

#[test]
fn packed_roundtrip_and_divides() {
    let layout = MonomialLayout::compile(&MonomialOrder::Lex, 2).unwrap();
    let d = layout.pack(&[1, 0]).unwrap();
    let t = layout.pack(&[2, 1]).unwrap();
    assert!(layout.packed_divides(&d, &t).unwrap());
    assert!(!layout.packed_divides(&t, &d).unwrap());
    assert_eq!(layout.unpack(&layout.lcm_packed(&d, &t).unwrap()).unwrap(), vec![2, 1]);
}

#[test]
fn distinct_orders_distinct_layouts() {
    let lex = MonomialLayout::compile(&MonomialOrder::Lex, 2).unwrap();
    let weighted = MonomialLayout::compile(&MonomialOrder::Weighted { weights: vec![1, 10] }, 2).unwrap();
    let e1 = vec![1, 0];
    let e2 = vec![0, 1];
    assert_ne!(lex.cmp_exponents(&e1, &e2), weighted.cmp_exponents(&e1, &e2));
}

#[test]
fn pack_unpack_roundtrip_16bit() {
    let layout = MonomialLayout::compile(&MonomialOrder::Lex, 3).unwrap();
    assert_eq!(layout.bits_per_exponent(), 16);
    assert_eq!(layout.packed_words_per_monomial(), 1);
    let exp = vec![1u32, 65535, 0];
    let packed = layout.pack(&exp).unwrap();
    assert_eq!(layout.unpack(&packed).unwrap(), exp);
}

#[test]
fn cmp_packed_matches_unpacked() {
    let layout = MonomialLayout::compile(&MonomialOrder::GrLex, 2).unwrap();
    let a = layout.pack(&[2, 0]).unwrap();
    let b = layout.pack(&[1, 1]).unwrap();
    assert_eq!(layout.cmp_packed(&a, &b).unwrap(), layout.cmp_exponents(&[2, 0], &[1, 1]));
}
