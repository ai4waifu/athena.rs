//! Solve 合同骨架：Constraint / Problem / SolutionSet / Coverage。

use athena_engine::{
    domains::solve::{
        BindingMap, BoundSymbol, Constraint, ConstraintSet, CoverageStatus, ExecutionLimits, ResumeKind, ResumeToken, SolutionBranch,
        SolutionSet, SolveDomain, SolveGoal, SolvePolicy, SolveProblem, SolveRelationKind,
    },
    reasoning::solver::{DomainRef, SolverOperation, SolverRequest},
    runtime::ResultProviderId,
};
use athena_types::{AssumptionSetId, SymbolId, TermId};

#[test]
fn solve_problem_rejects_unknown_parameter_overlap() {
    let x = BoundSymbol::free(SymbolId(1));
    let err = SolveProblem::try_new(
        ConstraintSet::and(vec![Constraint::equation(TermId(10), TermId(11))]),
        vec![x],
        vec![x],
        SolveDomain::Reals,
        AssumptionSetId(0),
        SolveGoal::ExactSolutionSet,
        SolvePolicy::default(),
        ExecutionLimits::default(),
    )
    .expect_err("overlap must fail");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("unknown_parameter_overlap"));
}

#[test]
fn solve_problem_rejects_empty_unknowns_for_exact_goal() {
    let err = SolveProblem::try_new(
        ConstraintSet::empty_and(),
        Vec::new(),
        Vec::new(),
        SolveDomain::Complexes,
        AssumptionSetId(0),
        SolveGoal::ExactSolutionSet,
        SolvePolicy::default(),
        ExecutionLimits::default(),
    )
    .expect_err("empty unknowns must fail");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("empty_unknowns_for_goal"));
}

#[test]
fn coverage_gates_exact_union_find() {
    assert!(CoverageStatus::Complete.admits_exact_union_find());
    assert!(CoverageStatus::CompleteUnderAssumptions.admits_exact_union_find());
    assert!(!CoverageStatus::LocalOnly.admits_exact_union_find());
    assert!(!CoverageStatus::CertifiedSubset.admits_exact_union_find());
    assert!(!CoverageStatus::Probable.admits_exact_union_find());
    assert!(
        !CoverageStatus::ResourceLimited {
            frontier: ResumeToken::empty_with_provider(ResumeKind::Cut, ResultProviderId::POLYNOMIAL.stamped())
        }
        .admits_exact_union_find()
    );
    let stamped = ResumeToken::empty_with_provider(ResumeKind::Cut, ResultProviderId::POLYNOMIAL.stamped());
    assert!(stamped.accepts_provider(ResultProviderId::POLYNOMIAL.stamped()));
    assert!(!ResumeToken::empty(ResumeKind::Cut).accepts_provider(ResultProviderId::POLYNOMIAL.stamped()));
    assert!(CoverageStatus::LocalOnly.must_surface_to_renderer());
}

#[test]
fn resource_limited_solution_registers_on_session_frontier_store() {
    use athena_engine::runtime::{ResumeCheck, Session};

    let stamp = ResultProviderId::LINEAR_ALGEBRA.stamped();
    let token = ResumeToken::empty_with_provider(ResumeKind::LinearExact, stamp);
    let mut solution = SolutionSet {
        variables: vec![BoundSymbol::free(SymbolId(0))],
        branches: Vec::new(),
        coverage: CoverageStatus::ResourceLimited { frontier: token.owning_copy() },
        domain: SolveDomain::Rationals,
        proof: None,
        residual: None,
        frontier: Some(token),
        frontier_id: None,
    };

    let mut session = Session::new();
    let id = solution.register_frontier_on_session(&mut session, 0xDEAD_BEEF, Some("linear_exact")).expect("frontier id");
    assert_eq!(solution.frontier_id, Some(id));
    assert!(session.frontiers.contains(id));

    let stored = session.frontiers.get(id).expect("stored");
    assert_eq!(stored.goal_fingerprint, 0xDEAD_BEEF);
    assert_eq!(stored.algorithm, Some("linear_exact"));
    assert!(
        stored
            .validate_resume(ResumeCheck {
                provider: stamp,
                assumption_scope: None,
                goal_fingerprint: 0xDEAD_BEEF,
                plan_fingerprint: None,
                object_fingerprints: &[],
                available_certificates: &[],
                cancelled: false,
                budget_limit: None,
            })
            .is_ok()
    );
    // 第二次登记幂等。
    assert_eq!(solution.register_frontier_on_session(&mut session, 0xDEAD_BEEF, Some("linear_exact")), Some(id));
    assert_eq!(session.frontiers.count(), 1);
}

#[test]
fn find_instance_and_find_root_are_not_complete_sets() {
    let x = BoundSymbol::free(SymbolId(0));
    let mut bindings = BindingMap::empty();
    bindings.insert(x, TermId(42));
    let branch = SolutionBranch::candidate(bindings);

    let instance = SolutionSet::certified_subset(vec![x], SolveDomain::Integers, vec![branch.owning_copy()]);
    assert!(!instance.admits_exact_union_find());
    assert!(matches!(instance.coverage, CoverageStatus::CertifiedSubset));

    let local = SolutionSet::local_only(vec![x], SolveDomain::Reals, branch);
    assert!(!local.admits_exact_union_find());
    assert!(matches!(local.coverage, CoverageStatus::LocalOnly));
    assert!(SolveGoal::ModelFinding.is_inherently_local_or_partial());
    assert!(SolveGoal::LocalNumericalRoot.is_inherently_local_or_partial());
}

#[test]
fn solve_relation_kinds_are_not_equality() {
    assert_eq!(SolveRelationKind::Satisfies { solution: TermId(1), problem: TermId(2) }.name(), "Satisfies");
    assert!(!SolveRelationKind::Satisfies { solution: TermId(1), problem: TermId(2) }.drives_exact_rewrite());
    assert!(SolveRelationKind::CompleteFor { solution_set: TermId(3), problem: TermId(2) }.drives_exact_rewrite());
    assert!(!SolveRelationKind::LocalConvergence { root: TermId(4), policy_tag: "newton".into() }.drives_exact_rewrite());
}

#[test]
fn solver_request_remains_dispatch_only() {
    let req = SolverRequest {
        domain: DomainRef::Solve,
        roots: vec![TermId(1)],
        operation: SolverOperation { name: "candidate".into() },
        limits: Default::default(),
        assumptions: AssumptionSetId(0),
    };
    assert_eq!(req.domain, DomainRef::Solve);
    assert_eq!(req.operation.name, "candidate");
}
