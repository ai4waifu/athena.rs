//! 由 [`AdmissionJournal`] 派生、可重建的索引。

use crate::reasoning::mgraph::{
    core::types::RewriteWitness,
    equivalence::{
        congruence::CongruenceIndex,
        proof_forest::{ProofForest, ProofStepKind},
        union_find::ExactUnionFind,
    },
    facts::{
        claim::{Proposition, VerifiedClaim},
        journal::AdmissionJournal,
    },
    polynomial::POLYNOMIAL_PROVIDER_ID,
};

/// 派生索引（非真相源；可从 [`AdmissionJournal`] 全量重建）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivedIndexes {
    /// 无条件 exact 等式 union-find。
    pub exact_uf: ExactUnionFind,
    /// 已接纳等式的证明森林。
    pub proof_forest: ProofForest,
    /// 模同余 stable 指纹索引（与 `TermId` ExactUF 分离）。
    pub congruence: CongruenceIndex,
    /// admission 通过的 rewrite witness 边。
    pub rewrite_witnesses: Vec<RewriteWitness>,
}

impl DerivedIndexes {
    /// 从 [`AdmissionJournal`] 全量重建派生索引。
    pub fn rebuild_from(admission_journal: &AdmissionJournal) -> Self {
        let mut derived = Self::default();
        for claim in admission_journal.claims() {
            derived.apply_verified_claim(claim);
        }
        derived
    }

    /// 增量应用单条已验证 claim（更新 exact UF / proof forest / congruence / witness 索引）。
    pub fn apply_verified_claim(&mut self, claim: &VerifiedClaim) {
        if !claim.admissible_for_exact_union() {
            return;
        }
        match &claim.claim.proposition {
            Proposition::PolynomialResult { .. } => {
                self.rewrite_witnesses.push(RewriteWitness {
                    provider: POLYNOMIAL_PROVIDER_ID,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                });
            }
            Proposition::TermEquality { left, right } => {
                self.exact_uf.union(*left, *right);
                self.proof_forest.record(*left, *right, ProofStepKind::AdmittedEquality);
            }
            Proposition::Congruence {
                modulus_fingerprint,
                left,
                right,
            } => {
                // Fingerprint-space index only. Do not coerce fingerprints into `TermId` ProofForest edges.
                self.congruence.union(*modulus_fingerprint, *left, *right);
            }
            Proposition::CalculusRelation { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use athena_types::TermId;

    use crate::reasoning::mgraph::{
        facts::claim::{
            Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope, VerifiedClaim,
        },
        facts::journal::AdmissionJournal,
        core::types::CapabilityProviderId,
    };

    use super::*;

    fn term_eq_claim(left: u32, right: u32) -> VerifiedClaim {
        VerifiedClaim::from_admission(Claim {
            proposition: Proposition::TermEquality {
                left: TermId(left),
                right: TermId(right),
            },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::StructuralTermEquality {
                    left: TermId(left),
                    right: TermId(right),
                },
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
            proposition: Proposition::Congruence {
                modulus_fingerprint: modulus,
                left,
                right,
            },
            scope: Scope::Unconditional,
            guarantee: Guarantee::ProvenExact,
            evidence: Evidence::TrustedKernel {
                provider: CapabilityProviderId(0),
                certificate: EvidenceCertificate::TestHarness,
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
}
