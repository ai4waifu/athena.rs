//! Living `30` 统一 [`FrontierStore`]：可暂停 / 可恢复计算前沿外壳。
//!
//! 领域私有 payload 仅经 [`ResumeToken`]；禁止用字符串 label 冒充完成标志。

use std::collections::BTreeMap;

use athena_types::{AssumptionSetId, Diagnostic, DiagnosticCode, FrontierId};

use crate::{
    domains::solve::ResumeToken,
    runtime::results::ResultProviderStamp,
};

/// 统一前沿记录（goal / plan / objects / budget / certificates / resume）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputationFrontier {
    /// 目标指纹。
    pub goal_fingerprint: u64,
    /// 计划指纹（尚无 plan 时为 `None`）。
    pub plan_fingerprint: Option<u64>,
    /// 输入对象指纹集。
    pub object_fingerprints: Vec<u64>,
    /// 表示族标签（bootstrap · 非算法选择令牌）。
    pub representation: Option<&'static str>,
    /// 算法标签（bootstrap · Reflector 私有名，不得当 admission 证明）。
    pub algorithm: Option<&'static str>,
    /// 已消耗预算单位（语义由 provider 解释）。
    pub budget_consumed: u64,
    /// 中间证书指纹（可重放检查的句柄，非证明本身）。
    pub certificate_fingerprints: Vec<u64>,
    /// 假设作用域（resume 前须未变化）。
    pub assumption_scope: Option<AssumptionSetId>,
    /// 恢复令牌（含 provider 合同戳）。
    pub resume: ResumeToken,
}

impl ComputationFrontier {
    /// 最小前沿骨架（须自带已盖戳的 [`ResumeToken`]）。
    pub fn new(goal_fingerprint: u64, resume: ResumeToken) -> Self {
        Self {
            goal_fingerprint,
            plan_fingerprint: None,
            object_fingerprints: Vec::new(),
            representation: None,
            algorithm: None,
            budget_consumed: 0,
            certificate_fingerprints: Vec::new(),
            assumption_scope: None,
            resume,
        }
    }

    /// Provider 合同是否允许从此前沿恢复。
    pub fn accepts_provider(&self, current: ResultProviderStamp) -> bool {
        self.resume.accepts_provider(current)
    }

    /// Resume 门：provider 不兼容则返回结构化诊断。
    pub fn resume_provider_gate(&self, current: ResultProviderStamp) -> Result<(), Diagnostic> {
        if self.accepts_provider(current) {
            Ok(())
        } else {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "frontier")
                .detail("operation", "resume")
                .detail("reason", "provider_version_incompatible"))
        }
    }
}

/// [`FrontierId`] → [`ComputationFrontier`] 存储。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FrontierStore {
    next: u32,
    frontiers: BTreeMap<FrontierId, ComputationFrontier>,
}

impl FrontierStore {
    /// 空存储。
    pub fn new() -> Self {
        Self::default()
    }

    /// 插入前沿并返回身份。
    pub fn insert(&mut self, frontier: ComputationFrontier) -> FrontierId {
        let id = FrontierId(self.next);
        self.next = self.next.saturating_add(1);
        self.frontiers.insert(id, frontier);
        id
    }

    /// 读取载荷。
    pub fn get(&self, id: FrontierId) -> Option<&ComputationFrontier> {
        self.frontiers.get(&id)
    }

    /// 是否已分配。
    pub fn contains(&self, id: FrontierId) -> bool {
        self.frontiers.contains_key(&id)
    }

    /// 已分配条数。
    pub fn count(&self) -> usize {
        self.frontiers.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.frontiers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use athena_types::AssumptionSetId;

    use super::{ComputationFrontier, FrontierStore};
    use crate::{
        domains::solve::{ResumeKind, ResumeToken},
        runtime::results::ResultProviderId,
    };

    #[test]
    fn insert_and_get_frontier() {
        let stamp = ResultProviderId::POLYNOMIAL.stamped();
        let resume = ResumeToken::empty_with_provider(ResumeKind::UnivariateFactor, stamp);
        let mut frontier = ComputationFrontier::new(0xA11CE, resume);
        frontier.plan_fingerprint = Some(0xBEEF);
        frontier.object_fingerprints = vec![1, 2, 3];
        frontier.assumption_scope = Some(AssumptionSetId(9));
        frontier.budget_consumed = 4;

        let mut store = FrontierStore::new();
        let id = store.insert(frontier.clone());
        assert!(store.contains(id));
        assert_eq!(store.get(id), Some(&frontier));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn resume_gate_rejects_stale_provider() {
        let stamp = ResultProviderId::LINEAR_ALGEBRA.stamped();
        let frontier = ComputationFrontier::new(1, ResumeToken::empty_with_provider(ResumeKind::LinearExact, stamp));
        assert!(frontier.resume_provider_gate(stamp).is_ok());
        let stale = ResultProviderId::LINEAR_ALGEBRA.stamped();
        let stale = crate::runtime::results::ResultProviderStamp { id: stale.id, version: 0 };
        let err = frontier.resume_provider_gate(stale).expect_err("stale");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("provider_version_incompatible"));
    }
}
