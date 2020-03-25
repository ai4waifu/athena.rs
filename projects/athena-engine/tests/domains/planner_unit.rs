//! 自 `src/domains/planner.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        calculus::{CalculusRequest, DerivativeOrder},
        dispatch::DomainRequest,
        *,
    },
};
use athena_types::{AssumptionSet, SymbolId, TermId};

#[test]
fn calculus_goal_gets_normalize_provider_verify_materialize() {
    let request = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression: TermId(0),
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let plan = plan_domain(&request);
    assert_eq!(
        plan.steps,
        vec![PlanStep::Normalize, PlanStep::SelectRepresentation, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult,]
    );
}

#[test]
fn series_goal_inserts_cross_domain_view_step() {
    let request = DomainRequest::Calculus(CalculusRequest::Series {
        expression: TermId(0),
        variable: SymbolId(0),
        center: TermId(1),
        order: 2,
        assumptions: AssumptionSet::empty(),
    });
    let plan = plan_domain(&request);
    assert_eq!(
        plan.steps,
        vec![
            PlanStep::Normalize,
            PlanStep::SelectRepresentation,
            PlanStep::CallDomainProvider,
            PlanStep::CrossDomainView,
            PlanStep::Verify,
            PlanStep::MaterializeResult,
        ]
    );
}
