//! 多项式 SemanticReflector（Living `29`）。

use crate::reasoning::mgraph::{
    core::{predicates, MGraphView},
    obligation::{ProofObligation, Reflection, SemanticReflector},
};

/// 多项式缺口 Reflector：先查 M-Graph，再声明缺少 `PolynomialRef`。
#[derive(Debug, Default, Clone, Copy)]
pub struct PolynomialReflector;

impl SemanticReflector for PolynomialReflector {
    fn reflect(&self, obligation: &ProofObligation, view: &MGraphView<'_>) -> Reflection {
        if obligation.predicate != predicates::POLYNOMIAL_RESULT {
            return Reflection::Inconclusive;
        }
        if let Some(relation) = view.find_accepted_by_predicate(obligation.scope, obligation.predicate) {
            return Reflection::AlreadyKnown { relation };
        }
        if obligation.known_objects.is_empty() {
            return Reflection::NeedObject {
                object_kind: "PolynomialRef",
            };
        }
        // Object fingerprints present but DomainObject store / request lowering not wired yet.
        Reflection::NeedObject {
            object_kind: "PolynomialRef",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::mgraph::core::{MGraphCore, ScopeRef};

    #[test]
    fn polynomial_result_needs_object_when_empty() {
        let core = MGraphCore::new();
        let view = MGraphView::new(&core);
        let obligation = ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: Vec::new(),
        };
        match PolynomialReflector.reflect(&obligation, &view) {
            Reflection::NeedObject { object_kind } => assert_eq!(object_kind, "PolynomialRef"),
            other => panic!("expected NeedObject, got {other:?}"),
        }
    }
}
