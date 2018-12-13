//! Semantic core：实现层 [`MGraphCore`] + legacy [`FactLog`] + 可重建 [`DerivedIndexes`]。

use super::{
    claim::VerifiedClaim,
    core::{MGraphCore, MGraphView},
    derived::DerivedIndexes,
    fact_log::{FactId, FactLog},
    refs::RelationRef,
    relation_index::RelationRecord,
};

/// 数学语义状态（scoped relation index + 派生索引）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticCore {
    /// Scoped relation 索引与 admit/close 入口。
    pub core: MGraphCore,
    /// Legacy append-only 视图（与 `core.relation_index` 同步；当前双写）。
    pub fact_log: FactLog,
    /// 由 fact log 派生的索引（可 `rebuild_derived` 重建）。
    pub derived: DerivedIndexes,
}

impl SemanticCore {
    /// 空 semantic core。
    pub fn new() -> Self {
        Self::default()
    }

    /// 经 admission gate 接纳后写入 semantic core（唯一写入路径；双写 fact log）。
    pub fn commit(&mut self, claim: VerifiedClaim) -> FactId {
        let id = self.core.admit(claim.clone());
        self.fact_log.append(claim.clone());
        self.derived.apply_verified_claim(&claim);
        id
    }

    /// 已接纳关系条数。
    pub fn relation_count(&self) -> usize {
        self.core.relation_count()
    }

    /// 按 id 查关系记录。
    pub fn relation(&self, id: RelationRef) -> Option<&RelationRecord> {
        self.core.relation_index().get(id)
    }

    /// 只读查询视图。
    pub fn view(&self) -> MGraphView<'_> {
        MGraphView::new(&self.core)
    }

    /// 从 fact log 重建全部派生索引。
    pub fn rebuild_derived(&mut self) {
        self.derived = DerivedIndexes::rebuild_from(&self.fact_log);
    }
}
