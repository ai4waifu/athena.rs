//! Append-only admission journal（M-Graph 接纳事件唯一追加源）。
//!
//! [`crate::reasoning::mgraph::relations::index::RelationIndex`] 是可查询索引，
//! [`crate::reasoning::mgraph::relations::derived::DerivedIndexes`] 是可丢弃性能索引。
//! 二者均可由本 journal 重建，不得另立长期写入口。

use crate::reasoning::mgraph::facts::claim::VerifiedClaim;

/// 接纳事件 id（单调递增；对应 journal 下标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FactId(pub u64);

/// 已验证 claim 的 append-only journal。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdmissionJournal {
    claims: Vec<VerifiedClaim>,
}

impl AdmissionJournal {
    /// 空 journal。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加已验证 claim（单调增长，不可撤销）。仅 admission 路径可写。
    pub(crate) fn append(&mut self, claim: VerifiedClaim) -> FactId {
        let id = FactId(self.claims.len() as u64);
        self.claims.push(claim);
        id
    }

    /// 全部已验证 claim（只读）。
    pub fn claims(&self) -> &[VerifiedClaim] {
        &self.claims
    }

    /// 按 id 查 claim。
    pub fn get(&self, id: FactId) -> Option<&VerifiedClaim> {
        self.claims.get(id.0 as usize)
    }

    /// 已验证事实条数。
    pub fn count(&self) -> usize {
        self.claims.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}
