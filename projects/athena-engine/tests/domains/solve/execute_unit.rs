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
        SolveResult::Exact { term } => term,
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
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    let Some(TermNode::Collection { elements: branches, .. }) = session.arena.get(term)
    else {
        panic!("expected OrderedCollection, got {:?}", session.arena.get(term));
    };
    assert_eq!(branches.len(), 2);
}
