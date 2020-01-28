//! 微积分 SemanticReflector（Living `29`）。

use athena_types::{AssumptionSet, SymbolId, TermId};

use crate::{
    domains::{
        DomainRequest,
        calculus::{CalculusRequest, DerivativeOrder},
        planner::plan_domain,
    },
    reasoning::mgraph::{
        core::{MGraphView, predicates},
        obligation::{ProofObligation, Reflection, SemanticReflector},
    },
};

/// 微积分缺口 Reflector：先查 M-Graph，再 `NeedComputation`。
#[derive(Debug, Default, Clone, Copy)]
pub struct CalculusReflector;

impl CalculusReflector {
    /// Scaffold request from obligation known objects (placeholders when unbound).
    fn request_for(obligation: &ProofObligation) -> Option<DomainRequest> {
        let expression = obligation.known_objects.first().map(|o| TermId(o.fingerprint as u32)).unwrap_or(TermId(0));
        let variable = obligation.known_objects.get(1).map(|o| SymbolId(o.fingerprint as u32)).unwrap_or(SymbolId(0));
        if obligation.predicate == predicates::DERIVATIVE_OF {
            return Some(DomainRequest::Calculus(CalculusRequest::Derivative {
                expression,
                variable,
                order: DerivativeOrder::First,
                assumptions: AssumptionSet::empty(),
            }));
        }
        if obligation.predicate == predicates::INTEGRAL_OF {
            return Some(DomainRequest::Calculus(CalculusRequest::Integral { expression, variable, assumptions: AssumptionSet::empty() }));
        }
        if obligation.predicate == predicates::SERIES_EXPANSION {
            return Some(DomainRequest::Calculus(CalculusRequest::Series {
                expression,
                variable,
                center: TermId(0),
                order: 1,
                assumptions: AssumptionSet::empty(),
            }));
        }
        None
    }
}

impl SemanticReflector for CalculusReflector {
    fn reflect(&self, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection {
        if obligation.predicate != predicates::DERIVATIVE_OF
            && obligation.predicate != predicates::INTEGRAL_OF
            && obligation.predicate != predicates::SERIES_EXPANSION
        {
            return Reflection::Inconclusive;
        }
        if let Some(relation) = view.find_accepted(obligation.scope, obligation.predicate, &obligation.known_objects) {
            return Reflection::AlreadyKnown { relation };
        }
        match Self::request_for(obligation) {
            Some(request) => Reflection::NeedComputation { plan: plan_domain(&request) },
            None => Reflection::Inconclusive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domains::planner::PlanStep,
        reasoning::mgraph::core::{MGraphCore, ScopeRef},
    };

    #[test]
    fn derivative_obligation_needs_computation_when_empty() {
        let core = MGraphCore::new();
        let view = MGraphView::new(&core);
        let obligation = ProofObligation { predicate: predicates::DERIVATIVE_OF, scope: ScopeRef::UNCONDITIONAL, known_objects: Vec::new() };
        match CalculusReflector.reflect(&obligation, &view) {
            Reflection::NeedComputation { plan } => {
                assert_eq!(plan.steps, vec![PlanStep::Normalize, PlanStep::CallDomainProvider, PlanStep::Verify, PlanStep::MaterializeResult,]);
            }
            other => panic!("expected NeedComputation, got {other:?}"),
        }
    }
}
