//! 自 `src/reasoning/mgraph/admission/gate.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    reasoning::mgraph::{
        CalculusRelationKind, CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, SemanticCore,
        admission::*,
    },
};
use athena_types::TermId;

#[test]
fn admit_congruence_rebuilds_modulus_isolated_index() {
    let mut semantic = SemanticCore::new();
    let policy = VerificationPolicy::default();
    AdmissionGate::admit_congruence(&mut semantic, 7, 10, 20, &policy).expect("mod7");
    AdmissionGate::admit_congruence(&mut semantic, 11, 10, 30, &policy).expect("mod11");
    assert_eq!(semantic.derived.congruence.find(7, 10), semantic.derived.congruence.find(7, 20));
    assert_ne!(semantic.derived.congruence.find(7, 10), semantic.derived.congruence.find(7, 30));
    assert_eq!(semantic.derived.congruence.modulus_count(), 2);
}

#[test]
fn mismatched_calculus_certificate_is_rejected() {
    let claim = Claim {
        proposition: Proposition::CalculusRelation {
            kind: CalculusRelationKind::DerivativeOf,
            expression_fingerprint: 1,
            variable_fingerprint: 2,
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
fn structural_equality_certificate_must_match_terms() {
    let ok = Claim {
        proposition: Proposition::TermEquality { left: TermId(1), right: TermId(2) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(2) },
            summary: "ok".into(),
        },
    };
    assert!(matches!(EvidenceVerifier::verify(&ok, &VerificationPolicy::default()), AdmissionOutcome::Admitted(_)));
    let bad = Claim {
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(1), right: TermId(9) },
            summary: "bad".into(),
        },
        ..ok
    };
    assert!(matches!(
        EvidenceVerifier::verify(&bad, &VerificationPolicy::default()),
        AdmissionOutcome::Rejected { reason: AdmissionRejectReason::EvidenceMismatch, .. }
    ));
}
