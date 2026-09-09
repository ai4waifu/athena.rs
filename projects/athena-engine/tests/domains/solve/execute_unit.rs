//! 一元方程 `SolveRequest` 经 `DomainRequest` 执行。

use athena_engine::{
    Session,
    api::{AthenaRequest, DomainGoal},
    domains::{DomainRequest, context::DomainExecutionContext, solve::{SolveRequest, SolveResult, execute_solve}},
    execution::execute_ir_request,
};
use athena_ir::{Atom, SemanticOperator, TermNode};

#[test]
fn solve_x_squared_eq_one_via_execute_solve() {
    let mut session = Session::new();
    let (equation, unknown) = {
        let dc = DomainExecutionContext::new(&mut session);
        let x = dc.intern("x");
        let xs = dc.symbol_id(x);
        let x2 = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(2)]);
        let equation = dc.apply_semantic(SemanticOperator::Equal, vec![x2, dc.in_(1)]);
        (equation, x)
    };
    let result = execute_solve(&mut session, SolveRequest::UnivariateEquation { equation, unknown });
    let term = match result {
        SolveResult::Exact { term, coverage } => {
            assert_eq!(coverage, athena_engine::domains::solve::CoverageStatus::Complete);
            term
        }
        other => panic!("expected Exact rule list, got {other:?}"),
    };
    // `{{x -> -1}, {x -> 1}}` nested lists of Rule
    let Some(TermNode::Collection { elements: branches, .. }) = session.arena.get(term)
    else {
        panic!("expected OrderedCollection of branches, got {:?}", session.arena.get(term));
    };
    assert_eq!(branches.len(), 2);
    let mut roots = Vec::new();
    for branch in branches {
        let Some(TermNode::Collection { elements: rules, .. }) = session.arena.get(*branch)
        else {
            panic!("expected rule list branch, got {:?}", session.arena.get(*branch));
        };
        assert_eq!(rules.len(), 1);
        let Some(TermNode::Application {
            head: athena_ir::ApplicationHead::Semantic(SemanticOperator::Rule),
            arguments,
        }) = session.arena.get(rules[0])
        else {
            panic!("expected Rule, got {:?}", session.arena.get(rules[0]));
        };
        assert_eq!(arguments.len(), 2);
        assert!(matches!(session.arena.get(arguments[0]), Some(TermNode::Atom(Atom::Symbol(s))) if *s == unknown));
        let Some(TermNode::Atom(Atom::Number(n))) = session.arena.get(arguments[1])
        else {
            panic!("expected numeric root, got {:?}", session.arena.get(arguments[1]));
        };
        roots.push(n.as_exact_integer().expect("integer root"));
    }
    roots.sort();
    assert_eq!(roots, vec![-1, 1]);
}

#[test]
fn solve_x_squared_eq_one_via_domain_goal() {
    let mut session = Session::new();
    let (equation, unknown) = {
        let dc = DomainExecutionContext::new(&mut session);
        let x = dc.intern("x");
        let xs = dc.symbol_id(x);
        let x2 = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(2)]);
        let equation = dc.apply_semantic(SemanticOperator::Equal, vec![x2, dc.in_(1)]);
        (equation, x)
    };
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::Solve(SolveRequest::UnivariateEquation { equation, unknown })));
    let result_id = execute_ir_request(&mut session, request).expect("solve goal");
    let stored = session.results.get(result_id).expect("result");
    assert_eq!(stored.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    assert_eq!(stored.status, athena_types::ComputationStatus::Candidate);
    assert_eq!(stored.evidence.len(), 1);
    let term = stored.symbolic_term.expect("term");
    let Some(TermNode::Collection { elements: branches, .. }) = session.arena.get(term)
    else {
        panic!("expected OrderedCollection, got {:?}", session.arena.get(term));
    };
    assert_eq!(branches.len(), 2);
}

#[test]
fn solve_two_linear_equations_projects_rules() {
    let mut session = Session::new();
    let (equations, unknowns) = {
        let dc = DomainExecutionContext::new(&mut session);
        let x = dc.intern("x");
        let y = dc.intern("y");
        let xs = dc.symbol_id(x);
        let ys = dc.symbol_id(y);
        let eq1 = dc.apply_semantic(SemanticOperator::Equal, vec![dc.apply_semantic(SemanticOperator::Add, vec![xs, ys]), dc.in_(3)]);
        let eq2 = dc.apply_semantic(
            SemanticOperator::Equal,
            vec![dc.apply_semantic(SemanticOperator::Subtract, vec![xs, ys]), dc.in_(1)],
        );
        (vec![eq1, eq2], vec![x, y])
    };
    let result = execute_solve(&mut session, SolveRequest::LinearEquations { equations, unknowns: unknowns.clone() });
    let term = match result {
        SolveResult::Exact { term, coverage } => {
            assert_eq!(coverage, athena_engine::domains::solve::CoverageStatus::Complete);
            term
        }
        other => panic!("expected Exact rule list, got {other:?}"),
    };
    let Some(TermNode::Collection { elements: branches, .. }) = session.arena.get(term)
    else {
        panic!("expected OrderedCollection, got {:?}", session.arena.get(term));
    };
    assert_eq!(branches.len(), 1);
    let Some(TermNode::Collection { elements: rules, .. }) = session.arena.get(branches[0])
    else {
        panic!("expected rule list branch");
    };
    assert_eq!(rules.len(), 2);
    let mut got = Vec::new();
    for rule in rules {
        let Some(TermNode::Application {
            head: athena_ir::ApplicationHead::Semantic(SemanticOperator::Rule),
            arguments,
        }) = session.arena.get(*rule)
        else {
            panic!("expected Rule");
        };
        let Some(TermNode::Atom(Atom::Symbol(s))) = session.arena.get(arguments[0])
        else {
            panic!("expected symbol lhs");
        };
        let Some(TermNode::Atom(Atom::Number(n))) = session.arena.get(arguments[1])
        else {
            panic!("expected number rhs");
        };
        got.push((*s, n.as_exact_integer().expect("int")));
    }
    got.sort_by_key(|(s, _)| s.0);
    assert_eq!(got, vec![(unknowns[0], 2), (unknowns[1], 1)]);
}
