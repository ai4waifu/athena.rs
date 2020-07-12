//! 自 `src/reasoning/mgraph/admission/gate.rs` 迁出的原内联测试。
//!
//! 含架构风险复审验收：`StructuralTermEquality` 须经 `TermStore::structural_eq`，
//! 不得仅凭证书字段一致准入；`VerifiedClaim` / journal 写路径不可由集成测试伪造。

use athena_engine::{
    Session,
    reasoning::mgraph::{
        CalculusRelationKind, CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, SemanticCore,
        VerificationContext, admission::*,
    },
};
use athena_ir::TermStore;
use athena_types::TermId;

#[test]
fn admit_congruence_rebuilds_modulus_isolated_index() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    let policy = VerificationPolicy::default();
    // 10 ≡ 17 (mod 7)，10 ≡ 21 (mod 11)
    AdmissionGate::admit_congruence(&mut store, &mut semantic, 7, 10, 17, &policy).expect("mod7");
    AdmissionGate::admit_congruence(&mut store, &mut semantic, 11, 10, 21, &policy).expect("mod11");
    assert_eq!(semantic.derived.congruence.find(7, 10), semantic.derived.congruence.find(7, 17));
    assert_ne!(semantic.derived.congruence.find(7, 10), semantic.derived.congruence.find(7, 21));
    assert_eq!(semantic.derived.congruence.modulus_count(), 2);
}

#[test]
fn mismatched_calculus_certificate_is_rejected() {
    let claim = Claim {
        proposition: Proposition::CalculusRelation {
            kind: CalculusRelationKind::DerivativeOf,
            expression_fingerprint: 1,
            variable_fingerprint: 2,
            request_identity: 0,
            result_term: TermId(3),
        },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CALCULUS_PROVIDER_ID,
            certificate: EvidenceCertificate::CalculusExact {
                kind: CalculusRelationKind::DerivativeOf,
                expression_fingerprint: 1,
                variable_fingerprint: 2,
                request_identity: 0,
                result_term: TermId(99),
            },
            summary: "forged".into(),
        },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. } => {}
        other => panic!("expected EvidenceMismatch, got {other:?}"),
    }
}

#[test]
fn field_matched_calculus_certificate_still_rejected_without_recompute() {
    let claim = Claim {
        proposition: Proposition::CalculusRelation {
            kind: CalculusRelationKind::DerivativeOf,
            expression_fingerprint: 1,
            variable_fingerprint: 2,
            request_identity: 7,
            result_term: TermId(3),
        },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CALCULUS_PROVIDER_ID,
            certificate: EvidenceCertificate::CalculusExact {
                kind: CalculusRelationKind::DerivativeOf,
                expression_fingerprint: 1,
                variable_fingerprint: 2,
                request_identity: 7,
                result_term: TermId(3),
            },
            summary: "field-only".into(),
        },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, .. } => {}
        other => panic!("expected NotExact without recompute, got {other:?}"),
    }
}

#[test]
fn congruence_field_only_certificate_is_rejected() {
    let claim = Claim {
        proposition: Proposition::Congruence { modulus_fingerprint: 7, left: 10, right: 20 },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::CongruenceExact { modulus_fingerprint: 7, left: 10, right: 20 },
            summary: "field-only".into(),
        },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, .. } => {}
        other => panic!("expected NotExact for field-only congruence, got {other:?}"),
    }
}

#[test]
fn verify_congruence_admits_residue_class_and_rejects_incongruent() {
    match EvidenceVerifier::verify_congruence(7, 10, 17, &VerificationPolicy::default()) {
        AdmissionOutcome::Admitted(_) => {}
        other => panic!("10 ≡ 17 (mod 7) must admit, got {other:?}"),
    }
    match EvidenceVerifier::verify_congruence(7, 10, 20, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, .. } => {}
        other => panic!("10 ≢ 20 (mod 7) must reject, got {other:?}"),
    }
}

#[test]
fn verify_calculus_admits_honest_derivative_and_rejects_forged_term() {
    use athena_engine::domains::{
        calculus::{CalculusRequest, DerivativeOrder, execute_calculus},
        context::DomainExecutionContext,
    };
    use athena_ir::SemanticOperator;
    use athena_types::AssumptionSet;

    let mut session = Session::new();
    let (expression, variable) = {
        let dc = DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(2)]);
        (expression, variable)
    };
    let request = CalculusRequest::Derivative {
        expression,
        variable,
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    };
    let honest = match execute_calculus(&mut session, request.owning_copy()) {
        athena_engine::domains::calculus::CalculusResult::Exact {
            value: athena_engine::domains::calculus::CalculusValue::Expression(t),
            ..
        } => t,
        other => panic!("expected Exact Expression, got {other:?}"),
    };
    match EvidenceVerifier::verify_calculus(
        &mut session,
        &request,
        CalculusRelationKind::DerivativeOf,
        honest,
        &VerificationPolicy::default(),
    ) {
        AdmissionOutcome::Admitted(_) => {}
        other => panic!("honest derivative must admit, got {other:?}"),
    }
    let forged = session.builder().number(athena_numeric::Number::small_int(999), Default::default());
    match EvidenceVerifier::verify_calculus(
        &mut session,
        &request,
        CalculusRelationKind::DerivativeOf,
        forged,
        &VerificationPolicy::default(),
    ) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, .. } => {}
        other => panic!("forged result must reject, got {other:?}"),
    }
}

#[test]
fn test_harness_rejected_without_policy_flag() {
    let claim = Claim {
        proposition: Proposition::TermEquality { left: TermId(1), right: TermId(1) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::TestHarness,
            summary: "harness".into(),
        },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. } => {}
        other => panic!("expected EvidenceMismatch, got {other:?}"),
    }
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::for_test_harness()) {
        AdmissionOutcome::Admitted(_) => {}
        other => panic!("expected Admitted under test harness policy, got {other:?}"),
    }
}

#[test]
fn structural_equality_certificate_must_match_proposition_fields() {
    let bad = Claim {
        proposition: Proposition::TermEquality { left: TermId(1), right: TermId(2) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(9) },
            summary: "bad".into(),
        },
    };
    assert!(matches!(
        EvidenceVerifier::verify(&bad, &VerificationPolicy::default()),
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. }
    ));
}

#[test]
fn structural_equality_without_term_store_is_rejected() {
    let claim = Claim {
        proposition: Proposition::TermEquality { left: TermId(0), right: TermId(0) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(0), right: TermId(0) },
            summary: "no-store".into(),
        },
    };
    match EvidenceVerifier::verify(&claim, &VerificationPolicy::default()) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, .. } => {}
        other => panic!("expected NotExact without TermStore context, got {other:?}"),
    }
}

#[test]
fn forged_true_equals_false_structural_equality_is_rejected() {
    let mut session = Session::new();
    let lhs = session.builder().boolean(true, Default::default());
    let rhs = session.builder().boolean(false, Default::default());
    assert_ne!(lhs, rhs);
    let forged = Claim {
        proposition: Proposition::TermEquality { left: lhs, right: rhs },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: lhs, right: rhs },
            summary: "forged-true-eq-false".into(),
        },
    };
    let err = AdmissionGate::admit_claim(&mut session.arena, &mut session.mgraph.semantic, forged, &VerificationPolicy::default(), None)
        .expect_err("true = false must not admit");
    assert_eq!(err, AdmissionRejectReason::NotExact);
    assert_eq!(session.mgraph.semantic.relation_count(), 0);
    assert_eq!(session.mgraph.semantic.admission_journal().count(), 0);
}

#[test]
fn structural_equality_of_identical_term_admits() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let claim = Claim {
        proposition: Proposition::TermEquality { left: t, right: t },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: t, right: t },
            summary: "reflexive".into(),
        },
    };
    AdmissionGate::admit_claim(&mut session.arena, &mut session.mgraph.semantic, claim, &VerificationPolicy::default(), None).expect("reflexive admit");
    assert_eq!(session.mgraph.semantic.relation_count(), 1);
}

#[test]
fn verify_in_rejects_false_equality_even_when_fields_match() {
    let mut session = Session::new();
    let lhs = session.builder().boolean(true, Default::default());
    let rhs = session.builder().boolean(false, Default::default());
    let claim = Claim {
        proposition: Proposition::TermEquality { left: lhs, right: rhs },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: lhs, right: rhs },
            summary: "fields-ok-math-false".into(),
        },
    };
    let mut ctx = VerificationContext::with_terms(&mut session.arena, None);
    match EvidenceVerifier::verify_in(&claim, &VerificationPolicy::default(), &mut ctx) {
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::NotExact, .. } => {}
        other => panic!("expected NotExact, got {other:?}"),
    }
}

#[test]
fn forged_typed_rewrite_without_rules_is_rejected() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    let claim = Claim {
        proposition: Proposition::TermEquality { left: TermId(0), right: TermId(1) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::TypedRewriteReplay {
                rule: athena_rewriter::RewriteRuleId(0),
                left: TermId(0),
                right: TermId(1),
            },
            summary: "forged-rewrite".into(),
        },
    };
    let err = AdmissionGate::admit_claim(&mut store, &mut semantic, claim, &VerificationPolicy::default(), None).expect_err("no rules");
    assert_eq!(err, AdmissionRejectReason::NotExact);
    assert_eq!(semantic.relation_count(), 0);
}

#[test]
fn rejected_certificate_never_admits() {
    let mut store = TermStore::new();
    let mut semantic = SemanticCore::new();
    let claim = Claim {
        proposition: Proposition::TermEquality { left: TermId(0), right: TermId(1) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::Rejected { guarantee: Guarantee::Unknown },
            summary: "rejected".into(),
        },
    };
    let err = AdmissionGate::admit_claim(&mut store, &mut semantic, claim, &VerificationPolicy::default(), None).expect_err("rejected cert");
    assert_eq!(err, AdmissionRejectReason::EvidenceMismatch);
    assert_eq!(semantic.relation_count(), 0);
}
