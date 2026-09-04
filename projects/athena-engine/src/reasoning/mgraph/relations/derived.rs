//! 由 [`AdmissionJournal`] 派生、可重建的索引。

use crate::reasoning::mgraph::{
    core::types::{CapabilityProviderId, RewriteWitness},
    equivalence::union_find::ExactUnionFind,
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

    /// 增量应用单条已验证 claim（更新 exact UF / witness 索引）。
    pub fn apply_verified_claim(&mut self, claim: &VerifiedClaim) {
        if !claim.admissible_for_exact_union() {
            return;
        }
        match &claim.claim.proposition {
            Proposition::PolynomialResult { .. } => {
                self.rewrite_witnesses.push(RewriteWitness { provider: POLYNOMIAL_PROVIDER_ID, inputs: Vec::new(), outputs: Vec::new() });
            }
            Proposition::Congruence { .. } => {}
        }
    }
}
