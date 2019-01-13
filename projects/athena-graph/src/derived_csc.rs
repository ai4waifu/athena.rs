//! 按需派生的 CSC 与相对 CSR 的失效合同。

use athena_ndarray::{ArrayStorage, InMemoryStorage, MemoryBudget};

use crate::{CscGraph, CsrGraph, GraphError, GraphId, GraphRevision, GraphStorageMetadata, RepresentationId, csr_to_csc};

/// 由 CSR 按需派生的 CSC；绑定源 `GraphId`/`GraphRevision`，源变更后不可用。
///
/// **不**与 CSR 默认双物化：调用方显式 [`DerivedCsc::from_csr`] 才构建。
#[derive(Debug)]
pub struct DerivedCsc<O = InMemoryStorage<u64>, I = InMemoryStorage<u64>> {
    source_graph_id: Option<GraphId>,
    source_revision: Option<GraphRevision>,
    csc: CscGraph<O, I>,
}

impl DerivedCsc<InMemoryStorage<u64>, InMemoryStorage<u64>> {
    /// 从 CSR 构建 CSC，并记录源身份/修订（取自 CSR metadata，若有）。
    pub fn from_csr<O: ArrayStorage<u64>, I: ArrayStorage<u64>>(
        csr: &CsrGraph<O, I>,
        budget: MemoryBudget,
    ) -> Result<Self, GraphError> {
        let mut csc = csr_to_csc(csr, budget)?;
        let (source_graph_id, source_revision, semantics, sorted) = match csr.metadata() {
            Some(m) => (m.graph_id, m.revision, m.semantics, m.sorted_adjacency),
            None => (None, None, None, true),
        };
        let mut meta = GraphStorageMetadata {
            representation_id: RepresentationId::CSC,
            graph_id: source_graph_id,
            revision: source_revision,
            semantics,
            sorted_adjacency: sorted,
            allows_duplicate_targets: csr.metadata().map(|m| m.allows_duplicate_targets).unwrap_or(false),
        };
        // 派生表示自身仍是 CSC；revision 表示「相对该逻辑修订有效」。
        if let (Some(gid), Some(rev), Some(sem)) = (source_graph_id, source_revision, semantics) {
            meta = meta.bind_snapshot(crate::GraphSnapshot::new(gid, rev, sem, RepresentationId::CSC));
        }
        csc.set_metadata(meta);
        Ok(Self { source_graph_id, source_revision, csc })
    }
}

impl<O, I> DerivedCsc<O, I> {
    /// 派生时所绑定的源图身份。
    pub const fn source_graph_id(&self) -> Option<GraphId> {
        self.source_graph_id
    }

    /// 派生时所绑定的源修订。
    pub const fn source_revision(&self) -> Option<GraphRevision> {
        self.source_revision
    }

    /// 底层 CSC（不校验时效）。
    pub const fn csc(&self) -> &CscGraph<O, I> {
        &self.csc
    }

    /// 相对给定 CSR 是否仍有效。
    pub fn is_valid_for<CO: ArrayStorage<u64>, CI: ArrayStorage<u64>>(&self, csr: &CsrGraph<CO, CI>) -> bool {
        match (self.source_graph_id, self.source_revision, csr.metadata()) {
            (Some(gid), Some(rev), Some(m)) => m.graph_id == Some(gid) && m.revision == Some(rev),
            // 无 metadata 的 CSR：仅当派生时也无绑定时视为「一次性匿名派生」，不保证跨 mutation。
            (None, None, None) => true,
            (None, None, Some(_)) => false,
            (Some(_), _, None) | (_, Some(_), None) => false,
            (None, Some(_), _) | (Some(_), None, _) => false,
        }
    }

    /// 校验有效后返回 CSC；过期则 [`GraphError::StaleCsc`]。
    pub fn ensure_valid_for<CO: ArrayStorage<u64>, CI: ArrayStorage<u64>>(
        &self,
        csr: &CsrGraph<CO, CI>,
    ) -> Result<&CscGraph<O, I>, GraphError> {
        if self.is_valid_for(csr) {
            Ok(&self.csc)
        }
        else {
            Err(GraphError::StaleCsc {
                derived_from: self.source_revision.unwrap_or(GraphRevision(0)),
                current: csr.metadata().and_then(|m| m.revision),
            })
        }
    }
}
