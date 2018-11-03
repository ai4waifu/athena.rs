//! 由 [`FactLog`] 派生、可重建的索引。

use super::{
    claim::{Proposition, VerifiedClaim},
    exact_uf::ExactUnionFind,
    fact_log::FactLog,
    polynomial::POLYNOMIAL_SOLVER_ID,
    types::RewriteWitness,
};

/// 派生索引（非真相源；可从 fact log 全量重建）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivedIndexes {
    /// 无条件 exact 等式 union-find。
    pub exact_uf: ExactUnionFind,
    /// admission 通过的 rewrite witness 边。
    pub rewrite_witnesses: Vec<RewriteWitness>,
}

impl DerivedIndexes {
    /// 从 fact log 全量重建派生索引。
    pub fn rebuild_from(fact_log: &FactLog) -> Self {
        let mut derived = Self::default();
        for claim in fact_log.claims() {
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
                self.rewrite_witnesses.push(RewriteWitness {
                    solver: POLYNOMIAL_SOLVER_ID,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                });
            }
        }
    }
}
