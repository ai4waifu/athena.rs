//! F4 Macaulay `Array2d` 测试。

use athena_engine::domains::polynomial::{
    CoefficientDomain, F4CriticalPair, F4UpdateComputation, F4UpdateLimits, MacaulayRowInput, MonomialOrder, PolynomialBuilder, RingTable,
    build_macaulay_matrix, eliminate_macaulay_column, f4_matrix_reduce_pairs, macaulay_matrix_polynomials, macaulay_row_to_polynomial,
    pair_sugar_degree, reduce_macaulay_matrix, resume_f4_basis_update, run_f4_basis_update, select_minimal_sugar_pairs,
    select_minimal_sugar_pairs_with, symbolic_preprocess_closure, symbolic_preprocess_pairs,
};
use athena_numeric::Number;
use athena_types::SymbolId;

#[test]
fn macaulay_array2d_two_rows_share_columns() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::small_int(1), vec![1]).unwrap(); // x
    b.push_term(Number::small_int(1), vec![0]).unwrap(); // + 1
    let f = b.build(&rings).unwrap();

    let zero = [0u32];
    let one = [1u32];
    let matrix = build_macaulay_matrix(
        &[MacaulayRowInput { multiplier: &zero, polynomial: &f }, MacaulayRowInput { multiplier: &one, polynomial: &f }],
        &rings,
    )
    .unwrap();

    assert_eq!(matrix.nrows(), 2);
    assert_eq!(matrix.ncols(), 3);
    assert_eq!(matrix.columns[0], vec![2]);
    assert_eq!(matrix.columns[1], vec![1]);
    assert_eq!(matrix.columns[2], vec![0]);
    assert_eq!(matrix.coeffs.shape().dimensions(), &[2, 3]);
}

#[test]
fn macaulay_rejects_empty_rows() {
    let rings = RingTable::new();
    let err = build_macaulay_matrix(&[], &rings).unwrap_err();
    assert_eq!(err.code.as_str(), "ATHENA_DOMAIN_ERROR");
}

#[test]
fn macaulay_row_roundtrips_to_polynomial() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::small_int(2), vec![1]).unwrap();
    b.push_term(Number::small_int(-3), vec![0]).unwrap();
    let f = b.build(&rings).unwrap();
    let zero = [0u32];
    let matrix = build_macaulay_matrix(&[MacaulayRowInput { multiplier: &zero, polynomial: &f }], &rings).unwrap();
    let back = macaulay_row_to_polynomial(&matrix, 0, &rings).unwrap();
    assert_eq!(back, f);
}

#[test]
fn macaulay_eliminate_column_over_q() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    b.push_term(Number::small_int(1), vec![0]).unwrap();
    let f = b.build(&rings).unwrap();
    let zero = [0u32];
    let one = [1u32];
    let matrix = build_macaulay_matrix(
        &[MacaulayRowInput { multiplier: &zero, polynomial: &f }, MacaulayRowInput { multiplier: &one, polynomial: &f }],
        &rings,
    )
    .unwrap();
    let reduced = eliminate_macaulay_column(&matrix, 1, &rings).unwrap();
    assert_eq!(reduced.nrows(), 2);
    let r0 = macaulay_row_to_polynomial(&reduced, 0, &rings).unwrap();
    let r1 = macaulay_row_to_polynomial(&reduced, 1, &rings).unwrap();
    assert_eq!(r0, f);
    let mut expect = PolynomialBuilder::new(q);
    expect.push_term(Number::small_int(1), vec![2]).unwrap();
    expect.push_term(Number::small_int(-1), vec![0]).unwrap();
    assert_eq!(r1, expect.build(&rings).unwrap());
}

#[test]
fn macaulay_reduce_matrix_over_q() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(q);
    b.push_term(Number::small_int(1), vec![1]).unwrap();
    b.push_term(Number::small_int(1), vec![0]).unwrap();
    let f = b.build(&rings).unwrap();
    let zero = [0u32];
    let one = [1u32];
    let matrix = build_macaulay_matrix(
        &[MacaulayRowInput { multiplier: &zero, polynomial: &f }, MacaulayRowInput { multiplier: &one, polynomial: &f }],
        &rings,
    )
    .unwrap();
    let reduced = reduce_macaulay_matrix(&matrix, &rings).unwrap();
    let polys = macaulay_matrix_polynomials(&reduced, &rings).unwrap();
    assert_eq!(polys.len(), 2);

    let mut expect_x2_minus_1 = PolynomialBuilder::new(q);
    expect_x2_minus_1.push_term(Number::small_int(1), vec![2]).unwrap();
    expect_x2_minus_1.push_term(Number::small_int(-1), vec![0]).unwrap();
    let x2m1 = expect_x2_minus_1.build(&rings).unwrap();

    let mut expect_x_plus_1 = PolynomialBuilder::new(q);
    expect_x_plus_1.push_term(Number::small_int(1), vec![1]).unwrap();
    expect_x_plus_1.push_term(Number::small_int(1), vec![0]).unwrap();
    let xp1 = expect_x_plus_1.build(&rings).unwrap();

    assert!(polys.iter().any(|p| p == &x2m1));
    assert!(polys.iter().any(|p| p == &xp1));
}

#[test]
fn f4_selects_minimal_sugar_pairs() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::GrLex).unwrap();
    let layout = &rings.get(q).unwrap().monomial_layout;

    let mut b0 = PolynomialBuilder::new(q);
    b0.push_term(Number::small_int(1), vec![1, 0]).unwrap(); // x
    b0.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let f0 = b0.build(&rings).unwrap();

    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![0, 1]).unwrap(); // y
    b1.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let f1 = b1.build(&rings).unwrap();

    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![2, 1]).unwrap(); // x^2 y
    b2.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let f2 = b2.build(&rings).unwrap();

    let basis = vec![f0, f1, f2];
    let selected = select_minimal_sugar_pairs(&basis, &[(0, 1), (0, 2)], layout).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].i, 0);
    assert_eq!(selected[0].j, 1);
    assert_eq!(selected[0].sugar, 2);
    assert_eq!(pair_sugar_degree(&basis[0], &basis[2], layout).unwrap(), 3);
}

#[test]
fn f4_pair_sugar_with_respects_inherited_sugar() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::GrLex).unwrap();
    let layout = &rings.get(q).unwrap().monomial_layout;

    let mut b0 = PolynomialBuilder::new(q);
    b0.push_term(Number::small_int(1), vec![1, 0]).unwrap();
    b0.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let f0 = b0.build(&rings).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![0, 1]).unwrap();
    b1.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let f1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![2, 1]).unwrap();
    b2.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let f2 = b2.build(&rings).unwrap();

    let basis = vec![f0, f1, f2];
    // Inflate sugar of f1 so natural-cheaper pair (0,1) loses to (0,2).
    let sugars = [1u32, 20, 3];
    let selected = select_minimal_sugar_pairs_with(&basis, &sugars, &[(0, 1), (0, 2)], layout).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].i, 0);
    assert_eq!(selected[0].j, 2);
}

#[test]
fn f4_matrix_step_reduces_x_plus_1_and_x2_minus_1() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();

    let mut b0 = PolynomialBuilder::new(q);
    b0.push_term(Number::small_int(1), vec![1]).unwrap();
    b0.push_term(Number::small_int(1), vec![0]).unwrap();
    let f0 = b0.build(&rings).unwrap();

    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0]).unwrap();
    let f1 = b1.build(&rings).unwrap();

    let basis = vec![f0.owning_copy(), f1.owning_copy()];
    let polys = f4_matrix_reduce_pairs(&basis, &[(0, 1)], &rings).unwrap();
    assert_eq!(polys.len(), 2);

    let mut neg = PolynomialBuilder::new(q);
    neg.push_term(Number::small_int(-1), vec![1]).unwrap();
    neg.push_term(Number::small_int(-1), vec![0]).unwrap();
    let neg_xp1 = neg.build(&rings).unwrap();

    assert!(polys.iter().any(|p| p == &f0 || p == &neg_xp1));
    assert!(polys.iter().any(|p| p == &f1));
}

#[test]
fn f4_symbolic_closure_adds_reducer_rows() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let layout = &rings.get(q).unwrap().monomial_layout;

    let mut b0 = PolynomialBuilder::new(q);
    b0.push_term(Number::small_int(1), vec![1]).unwrap();
    b0.push_term(Number::small_int(1), vec![0]).unwrap();
    let f0 = b0.build(&rings).unwrap();

    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0]).unwrap();
    let f1 = b1.build(&rings).unwrap();

    let basis = vec![f0, f1];
    let pair = F4CriticalPair { i: 0, j: 1, sugar: 2 };
    let minimal = symbolic_preprocess_pairs(&basis, &[pair], layout).unwrap();
    let closed = symbolic_preprocess_closure(&basis, &[pair], layout).unwrap();
    assert_eq!(minimal.len(), 2);
    assert!(closed.len() > minimal.len());
    assert!(closed.iter().any(|r| r.poly_index == 0 && r.multiplier.iter().all(|&e| e == 0)));
}

#[test]
fn f4_basis_update_completes_when_pair_reduces_to_known_lms() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    // 首项不互素：x² 与 xy，判据 1 不会跳过该对。
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&rings).unwrap();

    match run_f4_basis_update(vec![g1, g2], &rings, F4UpdateLimits::default()).unwrap() {
        F4UpdateComputation::Complete { basis, matrix_steps } => {
            assert!(basis.len() >= 2);
            assert!(matrix_steps >= 1);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

#[test]
fn f4_basis_update_inserts_new_leading_monomial() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&rings).unwrap();

    match run_f4_basis_update(vec![g1, g2], &rings, F4UpdateLimits::default()).unwrap() {
        F4UpdateComputation::Complete { basis, matrix_steps } => {
            assert!(basis.len() >= 3, "expected new basis element, got {}", basis.len());
            assert!(matrix_steps >= 1);
            assert!(basis.iter().any(|p| p.terms().first().is_some_and(|t| t.exponents() == [1, 0])));
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

#[test]
fn f4_basis_update_respects_matrix_step_budget() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&rings).unwrap();

    match run_f4_basis_update(vec![g1, g2], &rings, F4UpdateLimits { max_matrix_steps: 0, max_basis_size: 128 }).unwrap() {
        F4UpdateComputation::Partial { pending_pairs, matrix_steps, .. } => {
            assert_eq!(matrix_steps, 0);
            assert_eq!(pending_pairs.len(), 1);
        }
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[test]
fn f4_resume_completes_after_zero_step_partial() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&rings).unwrap();

    let F4UpdateComputation::Partial { basis, pending_pairs, matrix_steps, .. } =
        run_f4_basis_update(vec![g1, g2], &rings, F4UpdateLimits { max_matrix_steps: 0, max_basis_size: 128 }).unwrap()
    else {
        panic!("expected Partial");
    };
    assert_eq!(matrix_steps, 0);
    match resume_f4_basis_update(basis, pending_pairs, None, matrix_steps, None, None, &rings, F4UpdateLimits::default()).unwrap() {
        F4UpdateComputation::Complete { basis, matrix_steps } => {
            assert!(basis.len() >= 2);
            assert!(matrix_steps >= 1);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

#[test]
fn f4_skips_coprime_leading_monomial_pairs_without_matrix_step() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![1, 0]).unwrap();
    b1.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let g1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![0, 1]).unwrap();
    b2.push_term(Number::small_int(1), vec![0, 0]).unwrap();
    let g2 = b2.build(&rings).unwrap();

    match run_f4_basis_update(vec![g1, g2], &rings, F4UpdateLimits::default()).unwrap() {
        F4UpdateComputation::Complete { basis, matrix_steps } => {
            assert_eq!(basis.len(), 2);
            assert_eq!(matrix_steps, 0);
        }
        other => panic!("expected Complete with zero matrix steps, got {other:?}"),
    }
}

#[test]
fn f4_skips_chain_criterion_pairs_without_matrix_step() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    // LM: x^2, y^2, xy — xy | lcm(x^2,y^2). Side pairs (0,2)/(1,2) already treated.
    let mut b0 = PolynomialBuilder::new(q);
    b0.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    let f0 = b0.build(&rings).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![0, 2]).unwrap();
    let f1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    let f2 = b2.build(&rings).unwrap();

    match resume_f4_basis_update(vec![f0, f1, f2], vec![(0, 1)], None, 3, None, None, &rings, F4UpdateLimits::default()).unwrap() {
        F4UpdateComputation::Complete { basis, matrix_steps } => {
            assert_eq!(basis.len(), 3);
            assert_eq!(matrix_steps, 3);
        }
        other => panic!("expected Complete with no new matrix steps, got {other:?}"),
    }
}

#[test]
fn f4_partial_frontier_carries_sugar_vector() {
    let mut rings = RingTable::new();
    let q = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
    let mut b1 = PolynomialBuilder::new(q);
    b1.push_term(Number::small_int(1), vec![2, 0]).unwrap();
    b1.push_term(Number::small_int(-1), vec![0, 1]).unwrap();
    let g1 = b1.build(&rings).unwrap();
    let mut b2 = PolynomialBuilder::new(q);
    b2.push_term(Number::small_int(1), vec![1, 1]).unwrap();
    b2.push_term(Number::small_int(-1), vec![0, 0]).unwrap();
    let g2 = b2.build(&rings).unwrap();

    let F4UpdateComputation::Partial { basis, pending_pairs, matrix_steps, sugars } =
        run_f4_basis_update(vec![g1, g2], &rings, F4UpdateLimits { max_matrix_steps: 0, max_basis_size: 128 }).unwrap()
    else {
        panic!("expected Partial");
    };
    assert_eq!(sugars.len(), basis.len());
    assert_eq!(matrix_steps, 0);
    match resume_f4_basis_update(basis, pending_pairs, None, matrix_steps, Some(sugars), None, &rings, F4UpdateLimits::default()).unwrap() {
        F4UpdateComputation::Complete { basis, matrix_steps } => {
            assert!(basis.len() >= 2);
            assert!(matrix_steps >= 1);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}
