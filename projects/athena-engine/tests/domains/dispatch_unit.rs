//! 自 `src/domains/dispatch.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue},
        context::DomainExecutionContext,
        *,
    },
};
use athena_ir::SemanticOperator;
use athena_types::{AssumptionSet, Diagnostic};

#[test]
fn series_plan_opens_series_polynomial_view() {
    let mut session = Session::new();
    let (expression, variable, center) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let center = dc.in_(0);
        let expression = dc.apply_semantic(SemanticOperator::Unary(athena_ir::UnaryFunction::Sin), vec![xs]);
        (expression, variable, center)
    };
    let result = execute_domain(
        &mut session,
        DomainRequest::Calculus(CalculusRequest::Series { expression, variable, center, order: 2, assumptions: AssumptionSet::empty() }),
    )
    .expect("execute");
    match result {
        DomainResult::Calculus(
            CalculusResult::Exact { value: CalculusValue::Series(r), .. }
            | CalculusResult::Conditional { value: CalculusValue::Series(r), .. }
            | CalculusResult::Unevaluated { expression: CalculusValue::Series(r), .. },
        ) => {
            assert!(SeriesPolynomialView::open(&session.series_objects, r).is_some());
        }
        other => panic!("expected series DomainResult, got {other:?}"),
    }
}
