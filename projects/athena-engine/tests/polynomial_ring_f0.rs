//! F0 多项式环身份：RingId · RingDescriptor · MonomialOrder。

use athena_engine::{CoefficientDomain, CoefficientParent, MonomialOrder, RingTable, SymbolId};

fn sample_ring(table: &mut RingTable, order: MonomialOrder) -> athena_engine::RingId {
    table.intern(CoefficientDomain::Integer, vec![SymbolId(1), SymbolId(2)], order).expect("valid ring")
}

#[test]
fn lex_grlex_grevlex_compare_smoke() {
    let order_lex = MonomialOrder::Lex;
    let e1 = vec![1u32, 0];
    let e2 = vec![0u32, 1];
    assert_eq!(order_lex.cmp_exponents(&e1, &e2, 2).unwrap(), std::cmp::Ordering::Greater);

    let order_grlex = MonomialOrder::GrLex;
    let a = vec![2, 0];
    let b = vec![1, 0];
    assert_eq!(order_grlex.cmp_exponents(&a, &b, 2).unwrap(), std::cmp::Ordering::Greater);

    let order_grevlex = MonomialOrder::GrevLex;
    let c = vec![1, 2];
    let d = vec![2, 1];
    assert!(order_grevlex.cmp_exponents(&c, &d, 2).unwrap() != std::cmp::Ordering::Equal);
}

#[test]
fn same_vars_different_order_distinct_rings() {
    let mut table = RingTable::new();
    let vars = vec![SymbolId(10), SymbolId(20)];
    let r_lex = table.intern(CoefficientDomain::Rational, vars.clone(), MonomialOrder::Lex).unwrap();
    let r_grlex = table.intern(CoefficientDomain::Rational, vars, MonomialOrder::GrLex).unwrap();
    assert_ne!(r_lex, r_grlex);
}

#[test]
fn duplicate_symbol_rejected() {
    let mut table = RingTable::new();
    let err = table.intern(CoefficientDomain::Integer, vec![SymbolId(1), SymbolId(1)], MonomialOrder::Lex).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_VARIABLE_MISMATCH");
}

#[test]
fn weighted_len_mismatch_rejected() {
    let mut table = RingTable::new();
    let err = table
        .intern(CoefficientDomain::Integer, vec![SymbolId(1), SymbolId(2)], MonomialOrder::Weighted { weights: vec![1] })
        .unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_POLYNOMIAL_ORDER_INVALID");
}

#[test]
fn approximate_real_rejected_from_exact_ring() {
    let mut table = RingTable::new();
    let err = table.intern(CoefficientDomain::ApproximateReal, vec![SymbolId(0)], MonomialOrder::Lex).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_NUMERIC_CONVERSION_FORBIDDEN");
}

#[test]
fn ring_table_intern_idempotent() {
    let mut table = RingTable::new();
    let r1 = sample_ring(&mut table, MonomialOrder::Lex);
    let r2 = sample_ring(&mut table, MonomialOrder::Lex);
    assert_eq!(r1, r2);
    assert_eq!(table.len(), 1);
    let desc = table.get(r1).expect("descriptor");
    assert_eq!(desc.variables.len(), 2);
    let CoefficientParent::Ring(coeff_ring) = desc.coefficients
    else {
        panic!("expected coefficient ring parent");
    };
    assert_eq!(coeff_ring, desc.coefficient_ring);
    assert!(matches!(table.coeff_rings().get(coeff_ring).unwrap().domain, CoefficientDomain::Integer));
}
