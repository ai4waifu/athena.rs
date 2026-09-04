//! Semantic core：实现层 [`MGraphCore`] + [`AdmissionJournal`] + 可重建索引。

use crate::reasoning::mgraph::{
    core::{MGraphCore, MGraphView, refs::RelationRef},
    facts::{
        claim::VerifiedClaim,
        journal::{AdmissionJournal, FactId},
    },
    relations::{derived::DerivedIndexes, index::RelationIndex, index::RelationRecord},
};

/// 数学语义状态（admission journal + scoped relation index + 派生索引）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticCore {
    /// Scoped relation 索引与 admit/close 入口（查询面，可从 journal 重建）。
    pub core: MGraphCore,
    /// 唯一追加的接纳事件源。`RelationIndex` / `DerivedIndexes` 均可由此重建。
    pub admission_journal: AdmissionJournal,
    /// 由 journal 派生的索引（可丢弃后重建）。
    pub derived: DerivedIndexes,
}

impl SemanticCore {
    /// 空 semantic core。
    pub fn new() -> Self {
        Self::default()
    }

    /// 仅由 [`crate::reasoning::mgraph::admission::gate::AdmissionGate`] 调用。
    ///
    /// 写入顺序：先 append journal（权威），再更新查询索引与派生索引。
    pub(crate) fn commit(&mut self, claim: VerifiedClaim) -> FactId {
        let id = self.admission_journal.append(claim.clone());
        let index_id = self.core.admit(claim.clone());
        debug_assert_eq!(id, index_id, "journal and relation index ids must stay aligned");
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

    /// 从 [`AdmissionJournal`] 重建查询索引与派生索引。
    pub fn rebuild_from_journal(&mut self) {
        self.core.replace_relation_index(RelationIndex::rebuild_from(&self.admission_journal));
        self.derived = DerivedIndexes::rebuild_from(&self.admission_journal);
    }

    /// 从 [`AdmissionJournal`] 重建派生索引（保留当前 relation index）。
    pub fn rebuild_derived(&mut self) {
        self.derived = DerivedIndexes::rebuild_from(&self.admission_journal);
    }
}
