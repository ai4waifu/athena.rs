//! 自 `src/reasoning/mgraph/relations/derived.rs` 迁出的原内联测试。

use athena_types::TermId;

use athena_engine::reasoning::mgraph::{
    ProofStepKind,
    core::types::CapabilityProviderId,
    facts::{
        claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerifiedClaim},
        journal::AdmissionJournal,
    },
};

use athena_engine::{Session, reasoning::mgraph::relations::*};

fn term_eq_claim(left: u32, right: u32) -> VerifiedClaim {
    VerifiedClaim::from_admission(Claim {
        proposition: Proposition::TermEquality { left: TermId(left), right: TermId(right) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(left), right: TermId(right) },
            summary: String::new(),
        },
    })
}

#[test]
fn rebuild_projects_term_equality_into_uf_and_proof_forest() {
    let mut journal = AdmissionJournal::new();
    journal.append(term_eq_claim(1, 2));
    journal.append(term_eq_claim(2, 3));
    let derived = DerivedIndexes::rebuild_from(&journal);
    assert_eq!(derived.exact_uf.find(TermId(1)), derived.exact_uf.find(TermId(3)));
    assert_eq!(derived.proof_forest.len(), 2);
    assert_eq!(derived.proof_forest.edges()[0].step_kind, ProofStepKind::AdmittedEquality);
}

fn congruence_claim(modulus: u64, left: u64, right: u64) -> VerifiedClaim {
    VerifiedClaim::from_admission(Claim {
        proposition: Proposition::Congruence { modulus_fingerprint: modulus, left, right },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::CongruenceExact { modulus_fingerprint: modulus, left, right },
            summary: String::new(),
        },
    })
}

#[test]
fn rebuild_projects_congruence_into_fingerprint_index() {
    let mut journal = AdmissionJournal::new();
    journal.append(congruence_claim(97, 10, 20));
    journal.append(congruence_claim(97, 20, 30));
    let derived = DerivedIndexes::rebuild_from(&journal);
    assert_eq!(derived.congruence.find(97, 10), derived.congruence.find(97, 30));
    assert_eq!(derived.congruence.union_count(), 2);
    assert!(derived.proof_forest.is_empty());
    assert_eq!(derived.exact_uf.union_count(), 0);
}

#[test]
fn rebuild_keeps_congruence_classes_per_modulus() {
    let mut journal = AdmissionJournal::new();
    journal.append(congruence_claim(7, 10, 20));
    journal.append(congruence_claim(11, 10, 30));
    let derived = DerivedIndexes::rebuild_from(&journal);
    assert_eq!(derived.congruence.find(7, 10), derived.congruence.find(7, 20));
    assert_ne!(derived.congruence.find(7, 10), derived.congruence.find(7, 30));
    assert_eq!(derived.congruence.modulus_count(), 2);
}

#[test]
fn proof_forest_step_kind_follows_term_equality_certificate() {
    let mut journal = AdmissionJournal::new();
    journal.append(VerifiedClaim::from_admission(Claim {
        proposition: Proposition::TermEquality { left: TermId(1), right: TermId(2) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::ApplicationCongruence { left: TermId(1), right: TermId(2) },
            summary: String::new(),
        },
    }));
    journal.append(VerifiedClaim::from_admission(Claim {
        proposition: Proposition::TermEquality { left: TermId(3), right: TermId(4) },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: CapabilityProviderId(0),
            certificate: EvidenceCertificate::TypedRewriteReplay { rule: athena_rewriter::RewriteRuleId(0), left: TermId(3), right: TermId(4) },
            summary: String::new(),
        },
    }));
    let derived = DerivedIndexes::rebuild_from(&journal);
    assert_eq!(derived.proof_forest.edges()[0].step_kind, ProofStepKind::Congruence);
    assert_eq!(derived.proof_forest.edges()[1].step_kind, ProofStepKind::TypedRewrite);
}
