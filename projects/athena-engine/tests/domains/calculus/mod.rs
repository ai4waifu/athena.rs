//! Calculus domain goals via typed `DomainRequest` (Living `27`/`28`).
//!
//! No Mathematica-shaped `ap("Plus")` / `try_calculus_request` fixtures.

use athena_engine::{
    AthenaEngine,
    domains::{
        DomainRequest, DomainResult,
        calculus::{CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder},
    },
};
use athena_ir::UnaryFunction;
use athena_testing::SessionFixture;
use athena_types::AssumptionSet;

#[test]
fn derivative_goal_power_rule() {
    let mut fx = SessionFixture::new();
    let (expression, x) = {
        let mut t = fx.terms();
        let x = t.intern("x");
        let xs = t.symbol("x");
        let three = t.integer(3);
        (t.power(xs, three), x)
    };
    let goal = fx.domain().derivative(expression, x, DerivativeOrder::First, AssumptionSet::empty());
    let athena_engine::api::DomainGoal::Dispatch(DomainRequest::Calculus(req)) = goal
    else {
        unreachable!()
    };
    assert!(matches!(req, CalculusRequest::Derivative { order: DerivativeOrder::First, .. }));
    let engine = AthenaEngine::new();
    let result = engine.execute_domain(fx.session_mut(), DomainRequest::Calculus(req)).expect("domain");
    match result {
        DomainResult::Calculus(CalculusResult::Exact { value: CalculusValue::Expression(term), .. })
        | DomainResult::Calculus(CalculusResult::Conditional { value: CalculusValue::Expression(term), .. }) => {
            assert!(fx.session().arena.get(term).is_some());
        }
        other => panic!("expected calculus expression, got {other:?}"),
    }
}

#[test]
fn unary_in_derivative_expression() {
    let mut fx = SessionFixture::new();
    let (expression, x) = {
        let mut t = fx.terms();
        let x = t.intern("x");
        let xs = t.symbol("x");
        (t.unary_function(UnaryFunction::Sin, xs), x)
    };
    let goal = fx.domain().derivative_first(expression, x);
    let athena_engine::api::DomainGoal::Dispatch(DomainRequest::Calculus(req)) = goal
    else {
        unreachable!()
    };
    let engine = AthenaEngine::new();
    let _ = engine.execute_domain(fx.session_mut(), DomainRequest::Calculus(req)).expect("sin'");
}
