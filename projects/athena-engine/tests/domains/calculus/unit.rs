//! 自 `src/domains/calculus/mod.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{calculus::*, context::DomainExecutionContext},
};
use athena_ir::SemanticOperator;
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
