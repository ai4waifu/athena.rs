//! 自 `src/reasoning/mgraph/obligation/mod.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        DomainRequest,
        calculus::{CalculusRequest, DerivativeOrder},
        planner::{PlanStep, plan_domain},
    },
    reasoning::mgraph::{MGraphView, PredicateId, ScopeRef, core::MGraphCore, obligation::*},
};
use athena_types::{AssumptionSet, SymbolId, TermId};

struct AlwaysCompute;

impl SemanticReflector for AlwaysCompute {
    fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
        let request = DomainRequest::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        Reflection::NeedComputation { plan: plan_domain(&request) }
    }
}

#[test]
fn need_computation_carries_living28_plan() {
    let core = MGraphCore::new();
    let view = MGraphView::new(&core);
    let obligation = ProofObligation { predicate: PredicateId(0), scope: ScopeRef::UNCONDITIONAL, known_objects: Vec::new() };
    match AlwaysCompute.reflect(&obligation, &view) {
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
