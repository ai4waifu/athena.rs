//! 自 `src/reasoning/mgraph/semantic_entry.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    api::request::DomainGoal,
    domains::{
        DomainRequest, DomainResult,
        calculus::{CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder},
        context::DomainExecutionContext,
    },
    reasoning::mgraph::*,
};
use athena_ir::SemanticOperator;
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, SymbolId, TermId};

#[test]
fn calculus_goal_computes_when_session_graph_empty() {
    use athena_ir::{Atom, TermNode};
    use athena_types::SourceSpan;

    let mut session = Session::new();
    let expression = session.arena.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))), SourceSpan::default());
    let goal = DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
        expression,
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    }));
    match execute_domain_goal(&mut session, goal).expect("ok") {
        DomainSemanticOutcome::Computed(DomainResult::Calculus(_)) => {}
        other => panic!("expected Computed calculus, got {other:?}"),
    }
}

#[test]
fn calculus_second_goal_is_already_known_after_exact_admit() {
    let mut session = Session::new();
    let (expression, variable) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let three = dc.in_(3);
        let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, three]);
        (expression, variable)
    };
    let make_goal = || {
        DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
            expression,
            variable,
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        }))
    };
    let first = execute_domain_goal(&mut session, make_goal()).expect("first");
    let DomainSemanticOutcome::Computed(DomainResult::Calculus(CalculusResult::Exact { value: CalculusValue::Expression(term), .. })) = first
    else {
        panic!("expected Exact Expression first, got {first:?}");
    };
    assert!(session.mgraph.semantic.relation_count() >= 1);
    let second = execute_domain_goal(&mut session, make_goal()).expect("second");
    match second {
        DomainSemanticOutcome::AlreadyKnown { relation } => {
            let replayed =
                domain_result_from_semantic_outcome(&session, DomainSemanticOutcome::AlreadyKnown { relation }).expect("materialize");
            assert_eq!(
                replayed,
                DomainResult::Calculus(CalculusResult::Exact { value: CalculusValue::Expression(term), conditions: Vec::new() })
            );
        }
        other => panic!("expected AlreadyKnown, got {other:?}"),
    }
}

#[test]
fn polynomial_goal_computes_when_request_carries_polynomial() {
    use athena_engine::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest};
    use athena_types::SymbolId;
    let mut session = Session::new();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
    let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
    let poly = session.polynomial_objects.intern(polynomial, &session.rings);
    let goal = DomainGoal::Dispatch(DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly }));
    match execute_domain_goal(&mut session, goal).expect("ok") {
        DomainSemanticOutcome::Computed(DomainResult::Polynomial(_)) => {}
        other => panic!("expected Computed polynomial, got {other:?}"),
    }
}

#[test]
fn polynomial_second_goal_is_already_known_after_mgraph_admit() {
    use athena_engine::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialRequest, PolynomialResult};
    let mut session = Session::new();
    let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
    let polynomial = PolynomialBuilder::new(ring).build(&session.rings).expect("zero poly");
    let poly_ref = session.polynomial_objects.intern(polynomial, &session.rings);
    let make_goal = || DomainGoal::Dispatch(DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref }));
    let first = execute_domain_goal(&mut session, make_goal()).expect("first");
    let DomainSemanticOutcome::Computed(DomainResult::Polynomial(PolynomialResult::Exact { value })) = first
    else {
        panic!("expected Exact polynomial first, got {first:?}");
    };
    assert!(session.mgraph.semantic.relation_count() >= 1);
    let second = execute_domain_goal(&mut session, make_goal()).expect("second");
    match second {
        DomainSemanticOutcome::AlreadyKnown { relation } => {
            let replayed =
                domain_result_from_semantic_outcome(&session, DomainSemanticOutcome::AlreadyKnown { relation }).expect("materialize");
            assert_eq!(replayed, DomainResult::Polynomial(PolynomialResult::Exact { value }));
        }
        other => panic!("expected AlreadyKnown, got {other:?}"),
    }
}
