//! 多项式 SemanticReflector。

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
        // `DomainPlan` 来自 DomainPlanner（Normalize → … → Materialize）。
        // 对象身份已由义务指纹携带；请求在执行时再绑定。
        let scaffold = DomainRequest::Polynomial(PolynomialRequest::Normalize { polynomial: crate::domains::polynomial::PolynomialRef(0) });
        Reflection::NeedComputation { plan: plan_domain(&scaffold) }
    }
}
