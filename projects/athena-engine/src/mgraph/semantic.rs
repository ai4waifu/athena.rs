//! Semantic core：`FactLog` + 可重建 `DerivedIndexes`。

use super::{
    claim::VerifiedClaim,
    derived::DerivedIndexes,
    fact_log::{FactId, FactLog},
};

/// 数学语义状态（单调 verified claims + 派生索引）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticCore {
    /// Append-only 已验证事实。
    pub fact_log: FactLog,
    /// 由 fact log 派生的索引（可 `rebuild_derived` 重建）。
    pub derived: DerivedIndexes,
}

impl SemanticCore {
    /// 空 semantic core。
    pub fn new() -> Self {
        Self::default()
    }

    /// 经 admission gate 接纳后写入 semantic core（唯一写入路径）。
    pub fn commit(&mut self, claim: VerifiedClaim) -> FactId {
        let id = self.fact_log.append(claim.clone());
        self.derived.apply_verified_claim(&claim);
        id
    }

    /// 从 fact log 重建全部派生索引。
    pub fn rebuild_derived(&mut self) {
        self.derived = DerivedIndexes::rebuild_from(&self.fact_log);
    }
}
