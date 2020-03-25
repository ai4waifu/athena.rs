//! 自 `src/domains/polynomial/groebner.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        algebra::{PropertyState, PropertyWitness},
        polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, RingTable, *},
    },
    runtime::values::numeric_clone::clone_number,
};
use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId, SymbolId};
use std::collections::HashSet;

fn poly(rings: &RingTable, ring: RingId, terms: &[(i64, Vec<u32>)]) -> Polynomial {
    let mut b = PolynomialBuilder::new(ring);
    for &(c, ref exp) in terms {
        b.push_term(Number::small_int(c), exp.clone()).unwrap();
    }
    b.build(rings).unwrap()
}

#[test]
fn chain_criterion_true_when_third_lm_divides_lcm_and_pairs_treated() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let layout = &rings.get(ring).unwrap().monomial_layout;
    // LM: x^2, y^2, xy — xy | lcm(x^2,y^2)=x^2y^2
    let basis = vec![poly(&rings, ring, &[(1, vec![2, 0])]), poly(&rings, ring, &[(1, vec![0, 2])]), poly(&rings, ring, &[(1, vec![1, 1])])];
    let pending = HashSet::new();
    assert!(chain_criterion_applies(&basis, 0, 1, &pending, layout).unwrap());
}

#[test]
fn chain_criterion_false_while_side_pairs_still_pending() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let layout = &rings.get(ring).unwrap().monomial_layout;
    let basis = vec![poly(&rings, ring, &[(1, vec![2, 0])]), poly(&rings, ring, &[(1, vec![0, 2])]), poly(&rings, ring, &[(1, vec![1, 1])])];
    let mut pending = HashSet::new();
    pending.insert(ordered_pair(0, 2));
    pending.insert(ordered_pair(1, 2));
    assert!(!chain_criterion_applies(&basis, 0, 1, &pending, layout).unwrap());
}
