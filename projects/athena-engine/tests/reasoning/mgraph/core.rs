//! M-Graph 实现层：`MGraphCore` · scope/relation 索引 · admit 路径。

use athena_engine::reasoning::mgraph::{
    AdmissionGate, Claim, ClosureSeeds, Evidence, Guarantee, MGraphCore, MGraphView, OuterCandidate, POLYNOMIAL_PROVIDER_ID, Proposition,
    RelationStatus, Scope, ScopeRef, ScopeRelationKind, SemanticCore, VerificationPolicy, scope_from_ref, scope_to_ref,
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
    let mut semantic = SemanticCore::new();
    let r0 = admit_ok(&mut semantic, sample_claim(1));
    let r1 = admit_ok(&mut semantic, sample_claim(2));
    assert_eq!(r0, athena_engine::reasoning::mgraph::FactId(0));
    assert_eq!(r1, athena_engine::reasoning::mgraph::FactId(1));
    assert_eq!(semantic.relation_count(), 2);
}

#[test]
fn relation_index_groups_by_scope() {
    let mut semantic = SemanticCore::new();
    admit_ok(&mut semantic, sample_claim(10));
    let view = semantic.view();
    let ids = view.relations_in_scope(ScopeRef::UNCONDITIONAL);
    assert_eq!(ids.len(), 1);
    let rec = view.relation(ids[0]).unwrap();
    assert_eq!(rec.scope, ScopeRef::UNCONDITIONAL);
    assert_eq!(rec.status, RelationStatus::Accepted);
    assert_eq!(rec.predicate, athena_engine::reasoning::mgraph::predicates::POLYNOMIAL_RESULT);
    assert_eq!(rec.theory, athena_engine::reasoning::mgraph::TheoryContextId::POLYNOMIAL);
    assert_eq!(rec.provider, Some(POLYNOMIAL_PROVIDER_ID));
    assert!(rec.subjects.is_empty());
}

#[test]
fn admit_maps_congruence_predicate() {
    let mut semantic = SemanticCore::new();
    let id = admit_ok(
        &mut semantic,
        Claim {
            proposition: Proposition::Congruence { modulus_fingerprint: 7, left: 1, right: 8 },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel { provider: POLYNOMIAL_PROVIDER_ID, summary: "test".into() },
        },
    );
    let view = semantic.view();
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
fn semantic_core_commit_syncs_core_and_admission_journal() {
    let mut semantic = SemanticCore::new();
    let id = admit_ok(&mut semantic, sample_claim(42));
    assert_eq!(semantic.admission_journal.count(), 1);
    assert_eq!(semantic.relation_count(), 1);
    assert!(semantic.relation(id).is_some());
    assert!(semantic.view().relation(id).is_some());
}

#[test]
fn outer_candidate_is_not_stored_in_core() {
    let semantic = SemanticCore::new();
    let _candidate = OuterCandidate::new(Claim {
        proposition: Proposition::PolynomialResult {
            operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
            request_fingerprint: 99,
        },
        scope: Scope::Unconditional,
        guarantee: Guarantee::Candidate,
        evidence: Evidence::TrustedKernel { provider: POLYNOMIAL_PROVIDER_ID, summary: "pending".into() },
    });
    assert_eq!(semantic.relation_count(), 0);
}

#[test]
fn close_seeds_placeholder_does_not_panic() {
    let mut semantic = SemanticCore::new();
    admit_ok(&mut semantic, sample_claim(1));
    semantic.core.close(&ClosureSeeds { scopes: vec![ScopeRef::UNCONDITIONAL] });
    assert_eq!(semantic.relation_count(), 1);
}

#[test]
fn candidate_guarantee_is_rejected_by_admission_gate() {
    let mut semantic = SemanticCore::new();
    let err = AdmissionGate::admit_claim(
        &mut semantic,
        Claim {
            proposition: Proposition::PolynomialResult {
                operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
                request_fingerprint: 1,
            },
            scope: Scope::Unconditional,
            guarantee: Guarantee::Candidate,
            evidence: Evidence::TrustedKernel { provider: POLYNOMIAL_PROVIDER_ID, summary: "forge".into() },
        },
        &VerificationPolicy::default(),
    )
    .unwrap_err();
    assert_eq!(err, athena_engine::reasoning::mgraph::AdmissionRejectReason::InsufficientGuarantee);
    assert_eq!(semantic.relation_count(), 0);
}

fn admit_ok(semantic: &mut SemanticCore, claim: Claim) -> athena_engine::reasoning::mgraph::FactId {
    AdmissionGate::admit_claim(semantic, claim, &VerificationPolicy::default()).expect("should admit")
}

fn sample_claim(fingerprint: u64) -> Claim {
    Claim {
        proposition: Proposition::PolynomialResult {
            operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
            request_fingerprint: fingerprint,
        },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel { provider: POLYNOMIAL_PROVIDER_ID, summary: "test".into() },
    }
}
