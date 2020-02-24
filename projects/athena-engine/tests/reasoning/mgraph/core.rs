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
    assert!(rec.witness.is_some());
    assert_eq!(
        rec.subjects,
        vec![athena_engine::reasoning::mgraph::SemanticRef::Object(athena_engine::reasoning::mgraph::ObjectRef::new(
            athena_engine::reasoning::mgraph::TheoryContextId::POLYNOMIAL,
            10
        ))]
    );
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
            evidence: Evidence::TrustedKernel {
                provider: POLYNOMIAL_PROVIDER_ID,
                certificate: athena_engine::reasoning::mgraph::EvidenceCertificate::CongruenceExact {
                    modulus_fingerprint: 7,
                    left: 1,
                    right: 8,
                },
                summary: "test".into(),
            },
        },
    );
    let view = semantic.view();
    let rec = view.relation(id).unwrap();
    assert_eq!(rec.predicate, athena_engine::reasoning::mgraph::predicates::CONGRUENCE);
    assert_eq!(rec.subjects.len(), 3);
    assert_eq!(
        rec.subjects[0],
        athena_engine::reasoning::mgraph::SemanticRef::Object(athena_engine::reasoning::mgraph::ObjectRef::new(
            athena_engine::reasoning::mgraph::TheoryContextId::CONGRUENCE,
            1
        ))
    );
}

#[test]
fn scope_index_refines_edge() {
    let mut core = MGraphCore::new();
    let a = ScopeRef(1);
    let b = ScopeRef(2);
    core.refine_scope(a, b).expect("refines");
    assert!(core.scope_index().refines(a, b));
    let view = MGraphView::new(&core);
    assert_eq!(view.scope_edges().len(), 1);
    assert_eq!(view.scope_edges()[0].kind, ScopeRelationKind::Refines);
}

#[test]
fn find_accepted_transports_along_refines() {
    use athena_engine::reasoning::mgraph::predicates;
    use athena_types::AssumptionSetId;

    let mut semantic = SemanticCore::new();
    let local = Scope::UnderAssumptions(AssumptionSetId(0));
    let local_ref = scope_to_ref(local);
    semantic.core.refine_scope(local_ref, ScopeRef::UNCONDITIONAL).expect("refines");

    admit_ok(&mut semantic, sample_claim(77));
    assert!(semantic.view().find_accepted_by_predicate(ScopeRef::UNCONDITIONAL, predicates::POLYNOMIAL_RESULT).is_some());
    assert!(semantic.view().find_accepted_by_predicate(local_ref, predicates::POLYNOMIAL_RESULT).is_some());
}

#[test]
fn find_accepted_does_not_transport_upward() {
    use athena_engine::reasoning::mgraph::predicates;
    use athena_types::AssumptionSetId;

    let mut semantic = SemanticCore::new();
    let local = Scope::UnderAssumptions(AssumptionSetId(1));
    let local_ref = scope_to_ref(local);
    semantic.core.refine_scope(local_ref, ScopeRef::UNCONDITIONAL).expect("refines");

    let mut claim = sample_claim(88);
    claim.scope = local;
    admit_ok(&mut semantic, claim);

    assert!(semantic.view().find_accepted_by_predicate(local_ref, predicates::POLYNOMIAL_RESULT).is_some());
    assert!(semantic.view().find_accepted_by_predicate(ScopeRef::UNCONDITIONAL, predicates::POLYNOMIAL_RESULT).is_none());
    assert!(semantic.view().relations_in_scope(ScopeRef::UNCONDITIONAL).is_empty());
}

#[test]
fn admit_into_state_wakes_matching_obligation() {
    use athena_engine::reasoning::mgraph::{MGraphState, ProofObligation, predicates};

    let mut state = MGraphState::new();
    state.operational.obligation_index.register(ProofObligation {
        predicate: predicates::POLYNOMIAL_RESULT,
        scope: ScopeRef::UNCONDITIONAL,
        known_objects: vec![],
    });
    let (id, wake) = AdmissionGate::admit_claim_into_state(&mut state, sample_claim(55), &VerificationPolicy::default()).expect("admit");
    assert_eq!(wake.wakes.len(), 1);
    assert_eq!(wake.wakes[0].relation, id);
    assert!(state.operational.obligation_index.is_empty());
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
        evidence: Evidence::TrustedKernel {
            provider: POLYNOMIAL_PROVIDER_ID,
            certificate: athena_engine::reasoning::mgraph::EvidenceCertificate::TestHarness,
            summary: "pending".into(),
        },
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
            evidence: Evidence::TrustedKernel {
                provider: POLYNOMIAL_PROVIDER_ID,
                certificate: athena_engine::reasoning::mgraph::EvidenceCertificate::TestHarness,
                summary: "forge".into(),
            },
        },
        &VerificationPolicy::default(),
    )
    .unwrap_err();
    assert_eq!(err, athena_engine::reasoning::mgraph::AdmissionRejectReason::InsufficientGuarantee);
    assert_eq!(semantic.relation_count(), 0);
}

#[test]
fn find_accepted_consults_compatible_peer_locally() {
    use athena_engine::reasoning::mgraph::predicates;
    use athena_types::AssumptionSetId;

    let mut semantic = SemanticCore::new();
    let a = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(10)));
    let b = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(11)));
    semantic.core.mark_scopes_compatible(a, b).expect("compatible");

    let mut claim = sample_claim(201);
    claim.scope = Scope::UnderAssumptions(AssumptionSetId(11));
    admit_ok(&mut semantic, claim);

    assert!(semantic.view().find_accepted_by_predicate(a, predicates::POLYNOMIAL_RESULT).is_some());
    assert!(semantic.view().find_accepted_by_predicate(ScopeRef::UNCONDITIONAL, predicates::POLYNOMIAL_RESULT).is_none());
}

#[test]
fn find_accepted_skips_incompatible_ancestor() {
    use athena_engine::reasoning::mgraph::predicates;
    use athena_types::AssumptionSetId;

    let mut semantic = SemanticCore::new();
    let local = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(12)));
    semantic.core.refine_scope(local, ScopeRef::UNCONDITIONAL).expect("refines");
    semantic.core.mark_scopes_incompatible(local, ScopeRef::UNCONDITIONAL).expect("incompatible");

    admit_ok(&mut semantic, sample_claim(202));
    assert!(semantic.view().find_accepted_by_predicate(ScopeRef::UNCONDITIONAL, predicates::POLYNOMIAL_RESULT).is_some());
    assert!(semantic.view().find_accepted_by_predicate(local, predicates::POLYNOMIAL_RESULT).is_none());
}

#[test]
fn registering_incompatible_after_compatible_is_rejected() {
    use athena_types::AssumptionSetId;

    let mut semantic = SemanticCore::new();
    let a = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(13)));
    let b = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(14)));
    semantic.core.mark_scopes_compatible(a, b).expect("compatible");
    let err = semantic.core.mark_scopes_incompatible(a, b).expect_err("merge conflict");
    assert_eq!(err.reason_key(), "compatible_and_incompatible");
    assert!(!semantic.core.scope_index().incompatible_with(a, b));
}

#[test]
fn find_accepted_skips_incompatible_peer() {
    use athena_engine::reasoning::mgraph::predicates;
    use athena_types::AssumptionSetId;

    let mut semantic = SemanticCore::new();
    let a = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(13)));
    let b = scope_to_ref(Scope::UnderAssumptions(AssumptionSetId(14)));
    semantic.core.mark_scopes_incompatible(a, b).expect("incompatible");

    let mut claim = sample_claim(203);
    claim.scope = Scope::UnderAssumptions(AssumptionSetId(14));
    admit_ok(&mut semantic, claim);

    assert!(semantic.view().find_accepted_by_predicate(a, predicates::POLYNOMIAL_RESULT).is_none());
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
        evidence: Evidence::TrustedKernel {
            provider: POLYNOMIAL_PROVIDER_ID,
            certificate: athena_engine::reasoning::mgraph::EvidenceCertificate::PolynomialExact {
                operation: athena_engine::domains::polynomial::PolynomialCacheOp::Add,
                request_fingerprint: fingerprint,
                input_hashes: vec![],
                groebner_steps: None,
            },
            summary: "test".into(),
        },
    }
}
