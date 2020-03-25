//! 自 `src/reasoning/mgraph/reflectors/calculus.rs` 迁出的原内联测试。

use athena_engine::{
    domains::planner::PlanStep,
    reasoning::mgraph::{CalculusReflector, MGraphCore, MGraphView, ProofObligation, Reflection, ScopeRef, SemanticReflector, predicates},
};

#[test]
fn derivative_obligation_needs_computation_when_empty() {
    let core = MGraphCore::new();
    let view = MGraphView::new(&core);
    let obligation = ProofObligation { predicate: predicates::DERIVATIVE_OF, scope: ScopeRef::UNCONDITIONAL, known_objects: Vec::new() };
    match CalculusReflector.reflect(&obligation, &view) {
        Reflection::NeedComputation { plan } => {
            assert_eq!(
                plan.steps,
                vec![
                    PlanStep::Normalize,
                    PlanStep::SelectRepresentation,
                    PlanStep::CallDomainProvider,
                    PlanStep::Verify,
                    PlanStep::MaterializeResult,
                ]
            );
        }
        other => panic!("expected NeedComputation, got {other:?}"),
    }
}
