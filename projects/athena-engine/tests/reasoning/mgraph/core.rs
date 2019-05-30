//! M-Graph 实现层：`MGraphCore` · scope/relation 索引 · admit 路径。

use athena_engine::reasoning::mgraph::{
    Claim, ClosureSeeds, Evidence, Guarantee, MGraphCore, MGraphView, OuterCandidate, POLYNOMIAL_SOLVER_ID, Proposition,
    RelationStatus, Scope, ScopeRef, ScopeRelationKind, SemanticCore, VerifiedClaim, scope_from_ref, scope_to_ref,
};

#[test]
fn scope_ref_roundtrip_unconditional() {
    let r = scope_to_ref(Scope::Unconditional);
    assert_eq!(r, ScopeRef::UNCONDITIONAL);
    assert_eq!(scope_from_ref(r), Scope::Unconditional);
}

#[test]
fn scope_ref_roundtrip_assumptions() {
    use athena_types::AssumptionSetId;
    let scope = Scope::UnderAssumptions(AssumptionSetId(3));
    let r = scope_to_ref(scope);
    assert_ne!(r, ScopeRef::UNCONDITIONAL);
    assert_eq!(scope_from_ref(r), scope);
}

#[test]
fn mgraph_core_admit_is_monotonic() {
    let mut core = MGraphCore::new();
    let r0 = core.admit(sample_verified(1));
    let r1 = core.admit(sample_verified(2));
    assert_eq!(r0, athena_engine::reasoning::mgraph::FactId(0));
    assert_eq!(r1, athena_engine::reasoning::mgraph::FactId(1));
    assert_eq!(core.relation_count(), 2);
}

#[test]
fn relation_index_groups_by_scope() {
    let mut core = MGraphCore::new();
    core.admit(sample_verified(10));
    let view = MGraphView::new(&core);
    let ids = view.relations_in_scope(ScopeRef::UNCONDITIONAL);
    assert_eq!(ids.len(), 1);
    let rec = view.relation(ids[0]).unwrap();
    assert_eq!(rec.scope, ScopeRef::UNCONDITIONAL);
    assert_eq!(rec.status, RelationStatus::Accepted);
    assert_eq!(rec.predicate, athena_engine::reasoning::mgraph::predicates::POLYNOMIAL_RESULT);
    assert!(rec.subjects.is_empty());
}

#[test]
fn admit_maps_congruence_predicate() {
    let mut core = MGraphCore::new();
    let id = core.admit(VerifiedClaim::new(Claim {
        proposition: Proposition::Congruence { modulus_fingerprint: 7, left: 1, right: 8 },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel { solver: POLYNOMIAL_SOLVER_ID, summary: "test".into() },
    }));
    let view = MGraphView::new(&core);
    let rec = view.relation(id).unwrap();
    assert_eq!(rec.predicate, athena_engine::reasoning::mgraph::predicates::CONGRUENCE);
}

#[test]
fn scope_index_refines_edge() {
    let mut core = MGraphCore::new();
    let a = ScopeRef(1);
    let b = ScopeRef(2);
    core.refine_scope(a, b);
    assert!(core.scope_index().refines(a, b));
    let view = MGraphView::new(&core);
    assert_eq!(view.scope_edges().len(), 1);
    assert_eq!(view.scope_edges()[0].kind, ScopeRelationKind::Refines);
}

#[test]
fn semantic_core_commit_syncs_core_and_fact_log() {
    let mut semantic = SemanticCore::new();
    let id = semantic.commit(sample_verified(42));
    assert_eq!(semantic.fact_log.count(), 1);
    assert_eq!(semantic.relation_count(), 1);
    assert!(semantic.relation(id).is_some());
    assert!(semantic.view().relation(id).is_some());
}

#[test]
fn outer_candidate_is_not_stored_in_core() {
    let core = MGraphCore::new();
    let _candidate = OuterCandidate::new(Claim {
        proposition: Proposition::PolynomialResult {
            operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
            request_fingerprint: 99,
        },
        scope: Scope::Unconditional,
        guarantee: Guarantee::Candidate,
        evidence: Evidence::TrustedKernel { solver: POLYNOMIAL_SOLVER_ID, summary: "pending".into() },
    });
    assert_eq!(core.relation_count(), 0);
}

#[test]
fn close_seeds_placeholder_does_not_panic() {
    let mut core = MGraphCore::new();
    core.admit(sample_verified(1));
    core.close(&ClosureSeeds { scopes: vec![ScopeRef::UNCONDITIONAL] });
    assert_eq!(core.relation_count(), 1);
}

fn sample_verified(fingerprint: u64) -> VerifiedClaim {
    VerifiedClaim::new(Claim {
        proposition: Proposition::PolynomialResult {
            operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
            request_fingerprint: fingerprint,
        },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel { solver: POLYNOMIAL_SOLVER_ID, summary: "test".into() },
    })
}
