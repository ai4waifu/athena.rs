//! 由 [`AdmissionJournal`] 派生、可重建的索引。

use crate::reasoning::mgraph::{
    core::types::RewriteWitness,
    equivalence::{
        congruence::CongruenceIndex,
        proof_forest::{ProofForest, ProofStepKind},
        union_find::ExactUnionFind,
    },
    facts::{
        claim::{Evidence, EvidenceCertificate, Proposition, VerifiedClaim},
        journal::AdmissionJournal,
    },
    polynomial::POLYNOMIAL_PROVIDER_ID,
};

/// 派生索引（非真相源；可从 [`AdmissionJournal`] 全量重建）。
///
/// **不**实现 [`Clone`]（语义容器；重建用 [`Self::rebuild_from`]）。
#[derive(Debug, Default, PartialEq, Eq)]
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
                self.rewrite_witnesses.push(RewriteWitness { provider: POLYNOMIAL_PROVIDER_ID, inputs: Vec::new(), outputs: Vec::new() });
            }
            Proposition::TermEquality { left, right } => {
                self.exact_uf.union(*left, *right);
                let step = proof_step_from_evidence(&claim.claim.evidence);
                self.proof_forest.record(*left, *right, step);
            }
            Proposition::Congruence { modulus_fingerprint, left, right } => {
                // 仅指纹空间索引。禁止把指纹强行当成 `TermId` ProofForest 边。
                self.congruence.union(*modulus_fingerprint, *left, *right);
            }
            Proposition::CalculusRelation { .. } => {}
        }
    }
}

fn proof_step_from_evidence(evidence: &Evidence) -> ProofStepKind {
    match evidence {
        Evidence::TrustedKernel { certificate, .. } => match certificate {
            EvidenceCertificate::ApplicationCongruence { .. } => ProofStepKind::Congruence,
            EvidenceCertificate::TypedRewriteReplay { .. } => ProofStepKind::TypedRewrite,
            EvidenceCertificate::StructuralTermEquality { .. }
            | EvidenceCertificate::TestHarness
            | EvidenceCertificate::PolynomialExact { .. }
            | EvidenceCertificate::Rejected { .. }
            | EvidenceCertificate::CalculusExact { .. }
            | EvidenceCertificate::CongruenceExact { .. } => ProofStepKind::AdmittedEquality,
        },
    }
}
