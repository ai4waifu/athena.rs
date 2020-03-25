//! 自 `src/reasoning/mgraph/admission/outer_admit.rs` 迁出的原内联测试。

use athena_ir::{Atom, TermNode};
use athena_types::SourceSpan;

use athena_engine::{
    Session,
    reasoning::mgraph::{
        CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, HyperEdge, MGraphState, Proposition, Scope, admission::*,
        drain_hyper_edges_to_outer_pool, predicates,
    },
};
use athena_ir::TermStore;

#[test]
fn structural_outer_pair_admits_and_clears_pool() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let a = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(7))), span);
    let b = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(7))), span);
    assert_eq!(a, b);

    let mut state = MGraphState::new();
    state.operational.hyper_edges.push(HyperEdge { nodes: vec![a, b], predicate: predicates::REWRITE_EQUIVALENT });
    assert_eq!(drain_hyper_edges_to_outer_pool(&store, &mut state).staged, 1);
    assert_eq!(state.operational.outer_candidates.len(), 1);

    let report = admit_outer_pool_if_structural(&store, &mut state, &VerificationPolicy::default());
    assert_eq!(report.admitted, 1);
    assert_eq!(report.retained, 0);
    assert!(state.operational.outer_candidates.is_empty());
    assert_eq!(state.semantic.derived.exact_uf.find(a), state.semantic.derived.exact_uf.find(b));
}

#[test]
fn unequal_outer_pair_stays_in_pool() {
    let mut store = athena_ir::TermStore::new();
    let span = SourceSpan::default();
    let x = store.symbols_mut().intern("x");
    let y = store.symbols_mut().intern("y");
    let left = store.push(TermNode::Atom(Atom::Symbol(x)), span);
    let right = store.push(TermNode::Atom(Atom::Symbol(y)), span);

    let mut state = MGraphState::new();
    state.operational.outer_candidates.push(OuterCandidate::new(Claim {
        proposition: Proposition::TermEquality { left, right },
        scope: Scope::Unconditional,
        guarantee: Guarantee::Candidate,
        evidence: Evidence::TrustedKernel {
            provider: OUTER_STRUCTURAL_PROVIDER_ID,
            certificate: EvidenceCertificate::Rejected { guarantee: Guarantee::Candidate },
            summary: "unequal".into(),
        },
    }));

    let report = admit_outer_pool_if_structural(&store, &mut state, &VerificationPolicy::default());
    assert_eq!(report.admitted, 0);
    assert_eq!(report.retained, 1);
    assert_eq!(state.operational.outer_candidates.len(), 1);
    assert_eq!(state.semantic.relation_count(), 0);
}
