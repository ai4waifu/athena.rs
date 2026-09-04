//! 线性 / 一元多项式 → 统一 SolutionSet，以及 IR 关系归一化。

use athena_engine::domains::{
    linear_algebra::{MatrixValue, SolveDisposition},
    polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialFactorLimits, RingTable, factor_univariate},
    solve::{
        BindingValue, BoundSymbol, ConstraintSet, CoverageStatus, ExecutionLimits, LinearSolveMode, RelationalOperators,
        SolveDomain, SolveGoal, SolvePolicy, SolveProblem, adapt_univariate_factorization, assemble_solve_problem,
        execute_linear_system_goal, normalize_relational_app, solve_linear_system_exact, solve_linear_system_machine,
        solve_univariate_polynomial_roots,
    },
};
use athena_ir::{TermBuilder, TermNode, TermStore};
use athena_numeric::{Integer, Number, Rational};
use athena_types::{AssumptionSetId, OperatorId, SourceSpan, SymbolId};

fn i(n: i64) -> Integer {
    Integer::from_i64(n)
}

fn q(n: i64, d: i64) -> Rational {
    Rational::new(Integer::from_i64(n), Integer::from_i64(d))
}

#[test]
fn exact_unique_linear_is_complete_solution_set() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let b = MatrixValue::from_integers_row_major(2, 1, vec![i(5), i(11)]).unwrap();
    let unknowns = vec![BoundSymbol::free(SymbolId(0)), BoundSymbol::free(SymbolId(1))];
    let adapted = solve_linear_system_exact(&a, &b, unknowns, SolveDomain::Rationals).unwrap();
    assert_eq!(adapted.disposition, SolveDisposition::Unique);
    assert!(matches!(adapted.solution.coverage, CoverageStatus::Complete));
    assert!(adapted.solution.admits_exact_union_find());
    assert_eq!(adapted.solution.branches.len(), 1);
    let branch = &adapted.solution.branches[0];
    let x0 = branch.bindings.get(&BoundSymbol::free(SymbolId(0))).unwrap();
    let x1 = branch.bindings.get(&BoundSymbol::free(SymbolId(1))).unwrap();
    assert_eq!(adapted.values.get(x0), Some(&BindingValue::Rational(q(1, 1))));
    assert_eq!(adapted.values.get(x1), Some(&BindingValue::Rational(q(2, 1))));
}

#[test]
fn exact_inconsistent_is_complete_empty() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap();
    let b = MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(0)]).unwrap();
    let unknowns = vec![BoundSymbol::free(SymbolId(0)), BoundSymbol::free(SymbolId(1))];
    let adapted = solve_linear_system_exact(&a, &b, unknowns, SolveDomain::Rationals).unwrap();
    assert_eq!(adapted.disposition, SolveDisposition::Inconsistent);
    assert!(matches!(adapted.solution.coverage, CoverageStatus::Complete));
    assert!(adapted.solution.branches.is_empty());
}

#[test]
fn exact_infinite_is_certified_subset_not_complete() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap();
    let b = MatrixValue::from_integers_row_major(2, 1, vec![i(2), i(4)]).unwrap();
    let unknowns = vec![BoundSymbol::free(SymbolId(0)), BoundSymbol::free(SymbolId(1))];
    let adapted = solve_linear_system_exact(&a, &b, unknowns, SolveDomain::Rationals).unwrap();
    assert!(matches!(adapted.disposition, SolveDisposition::Infinite { .. }));
    assert!(matches!(adapted.solution.coverage, CoverageStatus::CertifiedSubset));
    assert!(!adapted.solution.admits_exact_union_find());
    assert_eq!(adapted.solution.branches.len(), 1);
}

#[test]
fn machine_unique_is_local_only() {
    let a = MatrixValue::from_f64_row_major(2, 2, vec![3.0, 1.0, 1.0, 2.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(2, 1, vec![9.0, 8.0]).unwrap();
    let unknowns = vec![BoundSymbol::free(SymbolId(0)), BoundSymbol::free(SymbolId(1))];
    let adapted = solve_linear_system_machine(&a, &b, unknowns, SolveDomain::Reals, 1e-12).unwrap();
    assert_eq!(adapted.disposition, SolveDisposition::Unique);
    assert!(matches!(adapted.solution.coverage, CoverageStatus::LocalOnly));
    assert!(!adapted.solution.admits_exact_union_find());
    assert!(adapted.solution.residual.is_some());
}

#[test]
fn univariate_linear_root_complete() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(2), vec![1]).unwrap();
    b.push_term(Number::small_int(6), vec![0]).unwrap();
    let p = b.build(&rings).unwrap();
    let unknown = BoundSymbol::free(SymbolId(0));
    let adapted =
        solve_univariate_polynomial_roots(p, &rings, unknown, SolveDomain::Rationals, PolynomialFactorLimits::default())
            .unwrap();
    assert_eq!(
        adapted.factorization_completeness,
        athena_engine::domains::polynomial::PolynomialFactorizationCompleteness::Complete
    );
    assert!(matches!(adapted.solution.coverage, CoverageStatus::Complete));
    assert_eq!(adapted.solution.branches.len(), 1);
    let term = adapted.solution.branches[0].bindings.get(&unknown).unwrap();
    match adapted.values.get(term) {
        Some(BindingValue::Number(n)) => assert_eq!(n.to_render_string(), "-3"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn univariate_quadratic_irreducible_is_unsupported_solution() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![2]).unwrap();
    b.push_term(Number::small_int(1), vec![0]).unwrap();
    let p = b.build(&rings).unwrap();
    let f = factor_univariate(p.clone(), &rings, PolynomialFactorLimits::default()).unwrap();
    let adapted = adapt_univariate_factorization(&f, BoundSymbol::free(SymbolId(0)), SolveDomain::Rationals).unwrap();
    assert!(matches!(adapted.solution.coverage, CoverageStatus::Unsupported));
    assert!(!adapted.solution.admits_exact_union_find());
    assert!(adapted.solution.branches.is_empty());
}

#[test]
fn univariate_x2_minus_1_complete_two_roots() {
    let mut rings = RingTable::new();
    let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex).unwrap();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::small_int(1), vec![2]).unwrap();
    b.push_term(Number::small_int(-1), vec![0]).unwrap();
    let p = b.build(&rings).unwrap();
    let unknown = BoundSymbol::free(SymbolId(0));
    let adapted =
        solve_univariate_polynomial_roots(p, &rings, unknown, SolveDomain::Rationals, PolynomialFactorLimits::default())
            .unwrap();
    assert!(matches!(adapted.solution.coverage, CoverageStatus::Complete));
    assert_eq!(adapted.solution.branches.len(), 2);
}

#[test]
fn normalize_equal_keeps_lhs_rhs() {
    let mut arena = TermStore::new();
    let mut builder = TermBuilder::new(&mut arena);
    let lhs = builder.symbol_id(SymbolId(1), SourceSpan::default());
    let rhs = builder.number(Number::small_int(0), SourceSpan::default());
    let eq = builder.app(OperatorId(0), vec![lhs, rhs], SourceSpan::default());
    let ops = RelationalOperators::placeholder();
    let c = normalize_relational_app(&arena, eq, &ops).unwrap();
    match c {
        athena_engine::domains::solve::Constraint::Equation(e) => {
            assert_eq!(e.lhs, lhs);
            assert_eq!(e.rhs, rhs);
            assert!(e.span.is_some());
        }
        other => panic!("expected equation, got {other:?}"),
    }
    assert!(matches!(arena.get(eq), Some(TermNode::App { args, .. }) if args.len() == 2));
}

#[test]
fn assemble_problem_from_ir_and_dispatch_linear_goal() {
    let mut arena = TermStore::new();
    let mut builder = TermBuilder::new(&mut arena);
    let x = builder.symbol_id(SymbolId(0), SourceSpan::default());
    let zero = builder.number(Number::small_int(0), SourceSpan::default());
    let eq = builder.app(OperatorId(0), vec![x, zero], SourceSpan::default());
    let ops = RelationalOperators::placeholder();
    let unknowns = vec![BoundSymbol::free(SymbolId(0)), BoundSymbol::free(SymbolId(1))];
    let problem = assemble_solve_problem(
        &arena,
        &[eq],
        &ops,
        unknowns.clone(),
        Vec::new(),
        SolveDomain::Rationals,
        AssumptionSetId(0),
        SolveGoal::LinearSystemSolve,
        SolvePolicy::default(),
        ExecutionLimits::default(),
    )
    .unwrap();
    assert_eq!(problem.constraints.members.len(), 1);
    assert_eq!(problem.goal, SolveGoal::LinearSystemSolve);

    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(0), i(1)]).unwrap();
    let b = MatrixValue::from_integers_row_major(2, 1, vec![i(3), i(4)]).unwrap();
    let adapted = execute_linear_system_goal(&problem, &a, &b, LinearSolveMode::Exact).unwrap();
    assert!(matches!(adapted.solution.coverage, CoverageStatus::Complete));
}

#[test]
fn dispatch_rejects_goal_mismatch() {
    let problem = SolveProblem::try_new(
        ConstraintSet::empty_and(),
        vec![BoundSymbol::free(SymbolId(0))],
        Vec::new(),
        SolveDomain::Rationals,
        AssumptionSetId(0),
        SolveGoal::PolynomialRootSet,
        SolvePolicy::default(),
        ExecutionLimits::default(),
    )
    .unwrap();
    let a = MatrixValue::from_integers_row_major(1, 1, vec![i(1)]).unwrap();
    let b = MatrixValue::from_integers_row_major(1, 1, vec![i(1)]).unwrap();
    let err = execute_linear_system_goal(&problem, &a, &b, LinearSolveMode::Exact).expect_err("goal mismatch");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("goal_mismatch"));
}

#[test]
fn solve_goals_stay_distinct() {
    assert_ne!(SolveGoal::LinearSystemSolve, SolveGoal::PolynomialRootSet);
    assert!(!SolveGoal::LinearSystemSolve.is_inherently_local_or_partial());
    let _ = AssumptionSetId(0);
}
