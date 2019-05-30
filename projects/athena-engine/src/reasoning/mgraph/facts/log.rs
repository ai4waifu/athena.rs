//! Append-only 已验证事实日志（semantic core 真相源）。

use crate::reasoning::mgraph::facts::claim::VerifiedClaim;

/// 事实 id（单调递增；对应 fact log 下标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FactId(pub u64);

/// 已验证事实 append-only 日志。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactLog {
    claims: Vec<VerifiedClaim>,
}

impl FactLog {
    /// 空日志。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加已验证 claim（单调增长，不可撤销）。
    pub fn append(&mut self, claim: VerifiedClaim) -> FactId {
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
