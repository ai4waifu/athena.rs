//! 多项式 SemanticReflector（Living `29`）。

use crate::{
    domains::{DomainRequest, planner::plan_domain, polynomial::PolynomialRequest},
    reasoning::mgraph::{
        core::{MGraphView, predicates},
        obligation::{ProofObligation, Reflection, SemanticReflector},
    },
};

/// 多项式缺口 Reflector：先查 M-Graph；有 `PolynomialRef` 指纹则 `NeedComputation`。
#[derive(Debug, Default, Clone, Copy)]
pub struct PolynomialReflector;

impl SemanticReflector for PolynomialReflector {
    fn reflect(&self, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection {
        if obligation.predicate != predicates::POLYNOMIAL_RESULT {
            return Reflection::Inconclusive;
        }
        if let Some(relation) = view.find_accepted(obligation.scope, obligation.predicate, &obligation.known_objects) {
            return Reflection::AlreadyKnown { relation };
        }
        if obligation.known_objects.is_empty() {
            return Reflection::NeedObject { object_kind: "PolynomialRef" };
        }
        // `DomainPlan` from DomainPlanner (Normalize → … → Materialize). Object identity
        // is already carried by the obligation fingerprint; request is rebound at execute.
        let scaffold = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: crate::domains::polynomial::PolynomialRef(0) });
        Reflection::NeedComputation { plan: plan_domain(&scaffold) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domains::planner::PlanStep,
        reasoning::mgraph::{MGraphCore, ObjectRef, ScopeRef, TheoryContextId},
    };

    #[test]
    fn polynomial_result_needs_object_when_empty() {
        let core = MGraphCore::new();
        let view = MGraphView::new(&core);
        let obligation =
            ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: Vec::new() };
        match PolynomialReflector.reflect(&obligation, &view) {
            Reflection::NeedObject { object_kind } => assert_eq!(object_kind, "PolynomialRef"),
            other => panic!("expected NeedObject, got {other:?}"),
        }
    }

    #[test]
    fn polynomial_result_needs_computation_when_object_present() {
        let core = MGraphCore::new();
        let view = MGraphView::new(&core);
        let obligation = ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![ObjectRef::new(TheoryContextId::POLYNOMIAL, 1)],
        };
        match PolynomialReflector.reflect(&obligation, &view) {
            Reflection::NeedComputation { plan } => {
                assert!(plan.steps.iter().any(|s| matches!(s, PlanStep::CallDomainProvider)));
            }
            other => panic!("expected NeedComputation, got {other:?}"),
        }
    }
}
