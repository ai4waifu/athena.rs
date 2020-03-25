//! Semantic core：实现层 [`MGraphCore`] + [`AdmissionJournal`] + 可重建索引。

use crate::reasoning::mgraph::{
    core::{MGraphCore, MGraphView, refs::RelationRef},
    facts::{
        claim::VerifiedClaim,
        journal::{AdmissionJournal, FactId},
        proof_dependency::ProofDependencyIndex,
    },
    relations::{
        derived::DerivedIndexes,
        index::{RelationIndex, RelationRecord},
    },
};

/// 数学语义状态（admission journal + scoped relation index + 派生索引）。
///
/// **不**实现 [`Clone`]（含 owning journal / relation 载荷）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SemanticCore {
    /// Scoped relation 索引与 admit/close 入口（查询面，可从 journal 重建）。
    pub core: MGraphCore,
    /// 唯一追加的接纳事件源。`RelationIndex` / `DerivedIndexes` 均可由此重建。
    pub admission_journal: AdmissionJournal,
    /// 由 journal 派生的索引（可丢弃后重建）。
    pub derived: DerivedIndexes,
    /// 已接纳事实的证明依赖（· 不随 `rebuild_derived` 丢弃）。
    pub proof_dependencies: ProofDependencyIndex,
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
        let id = self.admission_journal.append(claim.owning_copy());
        let index_id = self.core.admit(claim.owning_copy());
        debug_assert_eq!(id, index_id, "journal and relation index ids must stay aligned");
        self.derived.apply_verified_claim(&claim);
        id
    }

    /// 接纳后登记证明依赖（前提必须是更早的 `FactId`）。
    pub fn record_proof_dependencies(&mut self, fact: FactId, premises: &[FactId]) -> Result<(), athena_types::Diagnostic> {
        if self.admission_journal.get(fact).is_none() {
            return Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("domain", "mgraph")
                .detail("operation", "proof_dependency")
                .detail("reason", "unknown_fact")
                .detail("fact", fact.0.to_string()));
        }
        for premise in premises {
            if self.admission_journal.get(*premise).is_none() {
                return Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "mgraph")
                    .detail("operation", "proof_dependency")
                    .detail("reason", "unknown_premise")
                    .detail("premise", premise.0.to_string()));
            }
        }
        self.proof_dependencies.record(fact, premises)
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
