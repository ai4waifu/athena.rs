//! 自 `src/domains/calculus/mod.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{calculus::*, context::DomainExecutionContext},
    execution::execute_ir_request,
    api::{AthenaRequest, DomainGoal},
    domains::DomainRequest,
};
use athena_ir::{Atom, SemanticOperator, TermNode, UnaryFunction};
use athena_types::AssumptionSet;

#[test]
fn series_goal_interns_series_ref_into_session() {
    let mut session = Session::new();
    let (expression, variable, center) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let center = dc.in_(0);
        let expression = dc.apply_semantic(SemanticOperator::Unary(athena_ir::UnaryFunction::Sin), vec![xs]);
        (expression, variable, center)
    };
    let result =
        execute_calculus(&mut session, CalculusRequest::Series { expression, variable, center, order: 2, assumptions: AssumptionSet::empty() });
    match result {
        CalculusResult::Exact { value: CalculusValue::Series(r), .. }
        | CalculusResult::Conditional { value: CalculusValue::Series(r), .. }
        | CalculusResult::Unevaluated { expression: CalculusValue::Series(r), .. } => {
            assert!(session.series_objects.get(r).is_some());
            assert_eq!(session.series_objects.len(), 1);
        }
        other => panic!("expected SeriesRef payload, got {other:?}"),
    }
}

#[test]
fn integrate_reciprocal_yields_log() {
    let mut session = Session::new();
    let (expression, variable) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(-1)]);
        (expression, variable)
    };
    let result = execute_calculus(
        &mut session,
        CalculusRequest::Integral { expression, variable, assumptions: AssumptionSet::empty() },
    );
    match result {
        CalculusResult::Exact { value: CalculusValue::Expression(term), .. } => {
            match session.arena.get(term) {
                Some(TermNode::Application {
                    head: athena_ir::ApplicationHead::Semantic(op),
                    arguments,
                }) if op.as_unary() == Some(UnaryFunction::Log) && arguments.len() == 1 => {
                    assert!(matches!(session.arena.get(arguments[0]), Some(TermNode::Atom(Atom::Symbol(s))) if *s == variable));
                }
                other => panic!("expected Log[x], got {other:?}"),
            }
        }
        other => panic!("expected Exact Log, got {other:?}"),
    }
}

#[test]
fn integrate_x_sin_x_by_parts() {
    let mut session = Session::new();
    let (expression, variable) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let sin_x = dc.apply_semantic(SemanticOperator::Unary(UnaryFunction::Sin), vec![xs]);
        let expression = dc.apply_semantic(SemanticOperator::Multiply, vec![xs, sin_x]);
        (expression, variable)
    };
    let result = execute_calculus(
        &mut session,
        CalculusRequest::Integral { expression, variable, assumptions: AssumptionSet::empty() },
    );
    let term = match result {
        CalculusResult::Exact { value: CalculusValue::Expression(term), .. } => term,
        other => panic!("expected Exact by-parts antiderivative, got {other:?}"),
    };
    // `-x Cos[x] + Sin[x]` folds to an `Add` that still contains `Sin` and `Cos` of `x`.
    fn contains_unary(session: &Session, term: athena_types::TermId, uf: UnaryFunction, var: athena_types::SymbolId) -> bool {
        match session.arena.get(term) {
            Some(TermNode::Application {
                head: athena_ir::ApplicationHead::Semantic(op),
                arguments,
            }) if op.as_unary() == Some(uf)
                && arguments.len() == 1
                && matches!(session.arena.get(arguments[0]), Some(TermNode::Atom(Atom::Symbol(s))) if *s == var) =>
            {
                true
            }
            Some(TermNode::Application { arguments, .. }) => arguments.iter().any(|a| contains_unary(session, *a, uf, var)),
            _ => false,
        }
    }
    assert!(contains_unary(&session, term, UnaryFunction::Sin, variable), "missing Sin[x]: {:?}", session.arena.get(term));
    assert!(contains_unary(&session, term, UnaryFunction::Cos, variable), "missing Cos[x]: {:?}", session.arena.get(term));
    assert!(
        !matches!(
            session.arena.get(term),
            Some(TermNode::Application {
                head: athena_ir::ApplicationHead::Semantic(SemanticOperator::Integrate),
                ..
            })
        ),
        "still residual Integrate"
    );
}

#[test]
fn limit_reciprocal_at_positive_infinity_is_zero() {
    let mut session = Session::new();
    let (expression, variable) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(-1)]);
        (expression, variable)
    };
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Limit {
        expression,
        variable,
        approach: LimitApproach::PositiveInfinity,
        direction: LimitDirection::TwoSided,
        assumptions: AssumptionSet::empty(),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("limit");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
}

#[test]
fn derivative_order_two_of_x_squared() {
    let mut session = Session::new();
    let (expression, variable) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(2)]);
        (expression, variable)
    };
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Derivative {
        expression,
        variable,
        order: DerivativeOrder::Repeated(2),
        assumptions: AssumptionSet::empty(),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("d2");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
}
