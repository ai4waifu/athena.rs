//! 自 `src/reasoning/mgraph/reflectors/polynomial.rs` 迁出的原内联测试。

use athena_engine::{
    domains::planner::PlanStep,
    reasoning::mgraph::{
        MGraphCore, MGraphView, ObjectRef, PolynomialReflector, ProofObligation, Reflection, ScopeRef, SemanticReflector, TheoryContextId,
        predicates,
    },
};

#[test]
fn polynomial_result_needs_object_when_empty() {
    let core = MGraphCore::new();
    let view = MGraphView::new(&core);
    let obligation = ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: Vec::new() };
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
