//! 自 `src/reasoning/mgraph/closure/mod.rs` 迁出的原内联测试。

use athena_types::TermId;

use athena_engine::{
    Session,
    reasoning::mgraph::{
        AdmissionGate, CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, ProofForest, ProofStepKind, Proposition, Scope,
        VerificationPolicy, closure::*, core::state::MGraphState,
    },
};
use athena_ir::TermStore;

fn seed_equality(state: &mut MGraphState, left: u32, right: u32) {
    AdmissionGate::admit_claim(
        &mut state.semantic,
        Claim {
            proposition: Proposition::TermEquality { left: TermId(left), right: TermId(right) },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(left), right: TermId(right) },
                summary: "seed".into(),
            },
        },
        &VerificationPolicy::default(),
    )
    .expect("admit");
}

#[test]
fn empty_state_is_already_saturated() {
    let store = athena_ir::TermStore::new();
    let mut state = MGraphState::new();
    let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
    assert_eq!(result.stop, ClosureStopReason::Saturated);
    assert_eq!(result.steps_applied, 0);
    assert_eq!(result.hyper_edges_staged, 0);
    assert!(result.is_saturated());
}

#[test]
fn closure_materializes_transitivity_proof_edge() {
    let store = athena_ir::TermStore::new();
    let mut state = MGraphState::new();
    seed_equality(&mut state, 1, 2);
    seed_equality(&mut state, 2, 3);
    assert_eq!(state.semantic.derived.proof_forest.len(), 2);

    let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
    assert_eq!(result.stop, ClosureStopReason::Saturated);
    assert!(result.steps_applied >= 1);
    assert!(state.semantic.derived.proof_forest.edges().iter().any(|e| {
        e.step_kind == ProofStepKind::Transitivity
            && ((e.left == TermId(1) && e.right == TermId(3)) || (e.left == TermId(3) && e.right == TermId(1)))
    }));
    assert_eq!(state.semantic.derived.exact_uf.find(TermId(1)), state.semantic.derived.exact_uf.find(TermId(3)));
}

#[test]
fn step_budget_stops_before_saturation() {
    let store = athena_ir::TermStore::new();
    let mut state = MGraphState::new();
    seed_equality(&mut state, 1, 2);
    seed_equality(&mut state, 2, 3);
    seed_equality(&mut state, 3, 4);
    let result = run_closure_step(&store, &mut state, &ClosureLimits { max_steps: 1 });
    assert_eq!(result.stop, ClosureStopReason::StepBudget);
    assert_eq!(result.steps_applied, 1);
    assert!(!result.is_saturated());
}

#[test]
fn closure_drains_rewrite_hyper_edges_into_outer_pool() {
    use athena_engine::reasoning::mgraph::{HyperEdge, predicates};
    use athena_ir::{Atom, TermNode};
    use athena_types::SourceSpan;

    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let x = store.symbols_mut().intern("x");
    let y = store.symbols_mut().intern("y");
    let left = store.push(TermNode::Atom(Atom::Symbol(x)), span);
    let right = store.push(TermNode::Atom(Atom::Symbol(y)), span);

    let mut state = MGraphState::new();
    state.operational.hyper_edges.push(HyperEdge { nodes: vec![left, right], predicate: predicates::REWRITE_EQUIVALENT });
    let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
    assert_eq!(result.hyper_edges_staged, 1);
    assert_eq!(result.hyper_edges_retained, 0);
    assert_eq!(state.operational.outer_candidates.len(), 1);
    assert!(state.operational.hyper_edges.is_empty());
    assert_eq!(state.semantic.relation_count(), 0);
}
