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
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
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

/// Resume 校验输入（Living `30`：provider · scope · fingerprints · certificates · budget/cancel）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumeCheck<'a> {
    /// 当前 provider 合同戳。
    pub provider: ResultProviderStamp,
    /// 当前假设作用域。
    pub assumption_scope: Option<AssumptionSetId>,
    /// 当前 goal 指纹。
    pub goal_fingerprint: u64,
    /// 当前 plan 指纹。
    pub plan_fingerprint: Option<u64>,
    /// 当前对象指纹集。
    pub object_fingerprints: &'a [u64],
    /// 当前仍可重放的证书指纹（须覆盖冻结前沿中的每一项）。
    pub available_certificates: &'a [u64],
    /// 是否已取消。
    pub cancelled: bool,
    /// 预算上限（`None` = 不限额）。
    pub budget_limit: Option<u64>,
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

    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            goal_fingerprint: self.goal_fingerprint,
            plan_fingerprint: self.plan_fingerprint,
            object_fingerprints: self.object_fingerprints.clone(),
            representation: self.representation,
            algorithm: self.algorithm,
            budget_consumed: self.budget_consumed,
            certificate_fingerprints: self.certificate_fingerprints.clone(),
            assumption_scope: self.assumption_scope,
            resume: self.resume.owning_copy(),
        }
    }

    /// 附加中间证书指纹（通常来自 [`crate::reasoning::mgraph::WitnessRef`]）。
    pub fn push_certificate_fingerprint(&mut self, fingerprint: u64) {
        if !self.certificate_fingerprints.contains(&fingerprint) {
            self.certificate_fingerprints.push(fingerprint);
        }
    }

    /// 批量附加证书指纹。
    pub fn extend_certificate_fingerprints(&mut self, fingerprints: impl IntoIterator<Item = u64>) {
        for fingerprint in fingerprints {
            self.push_certificate_fingerprint(fingerprint);
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

    /// Resume 门：assumption scope 必须与冻结前沿一致（双方皆 `None` 视为无条件一致）。
    pub fn resume_assumption_gate(&self, current: Option<AssumptionSetId>) -> Result<(), Diagnostic> {
        if self.assumption_scope == current {
            Ok(())
        } else {
            Err(Diagnostic::new(DiagnosticCode::AssumptionUnresolved)
                .detail("domain", "frontier")
                .detail("operation", "resume")
                .detail("reason", "assumption_scope_changed"))
        }
    }

    /// Resume 门：goal / plan / object fingerprints 必须与冻结前沿一致。
    pub fn resume_fingerprint_gate(
        &self,
        goal_fingerprint: u64,
        plan_fingerprint: Option<u64>,
        object_fingerprints: &[u64],
    ) -> Result<(), Diagnostic> {
        if self.goal_fingerprint != goal_fingerprint {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "frontier")
                .detail("operation", "resume")
                .detail("reason", "goal_fingerprint_mismatch"));
        }
        if self.plan_fingerprint != plan_fingerprint {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "frontier")
                .detail("operation", "resume")
                .detail("reason", "plan_fingerprint_mismatch"));
        }
        if self.object_fingerprints.as_slice() != object_fingerprints {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "frontier")
                .detail("operation", "resume")
                .detail("reason", "object_fingerprints_mismatch"));
        }
        Ok(())
    }

    /// Resume 门：冻结证书指纹必须仍可在当前上下文重放（逐项覆盖，顺序无关）。
    pub fn resume_certificate_gate(&self, available_certificates: &[u64]) -> Result<(), Diagnostic> {
        for fingerprint in &self.certificate_fingerprints {
            if !available_certificates.contains(fingerprint) {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "frontier")
                    .detail("operation", "resume")
                    .detail("reason", "certificate_not_replayable")
                    .detail("missing_certificate", fingerprint.to_string()));
            }
        }
        Ok(())
    }

    /// Resume 门：未取消，且预算仍允许继续（已消耗严格小于上限）。
    pub fn resume_budget_gate(&self, cancelled: bool, budget_limit: Option<u64>) -> Result<(), Diagnostic> {
        if cancelled {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "frontier")
                .detail("operation", "resume")
                .detail("reason", "cancelled"));
        }
        if let Some(limit) = budget_limit {
            if self.budget_consumed >= limit {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "frontier")
                    .detail("operation", "resume")
                    .detail("reason", "budget_exhausted")
                    .detail("budget_consumed", self.budget_consumed.to_string())
                    .detail("budget_limit", limit.to_string()));
            }
        }
        Ok(())
    }

    /// 组合 resume 校验（provider · assumption · fingerprints · certificates · budget/cancel）。
    pub fn validate_resume(&self, check: ResumeCheck<'_>) -> Result<(), Diagnostic> {
        self.resume_provider_gate(check.provider)?;
        self.resume_assumption_gate(check.assumption_scope)?;
        self.resume_fingerprint_gate(check.goal_fingerprint, check.plan_fingerprint, check.object_fingerprints)?;
        self.resume_certificate_gate(check.available_certificates)?;
        self.resume_budget_gate(check.cancelled, check.budget_limit)
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

    use super::{ComputationFrontier, FrontierStore, ResumeCheck};
    use crate::{
        domains::solve::{ResumeKind, ResumeToken},
        runtime::results::ResultProviderId,
    };

    fn check<'a>(
        provider: crate::runtime::results::ResultProviderStamp,
        assumption_scope: Option<AssumptionSetId>,
        goal: u64,
        plan: Option<u64>,
        objects: &'a [u64],
        certs: &'a [u64],
        cancelled: bool,
        budget_limit: Option<u64>,
    ) -> ResumeCheck<'a> {
        ResumeCheck {
            provider,
            assumption_scope,
            goal_fingerprint: goal,
            plan_fingerprint: plan,
            object_fingerprints: objects,
            available_certificates: certs,
            cancelled,
            budget_limit,
        }
    }

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
        let id = store.insert(frontier.owning_copy());
        assert!(store.contains(id));
        assert_eq!(store.get(id), Some(&frontier));
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn resume_gate_rejects_stale_provider() {
        let stamp = ResultProviderId::LINEAR_ALGEBRA.stamped();
        let frontier = ComputationFrontier::new(1, ResumeToken::empty_with_provider(ResumeKind::LinearExact, stamp));
        assert!(frontier.resume_provider_gate(stamp).is_ok());
        let stale = crate::runtime::results::ResultProviderStamp { id: ResultProviderId::LINEAR_ALGEBRA, version: 0 };
        let err = frontier.resume_provider_gate(stale).expect_err("stale");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("provider_version_incompatible"));
    }

    #[test]
    fn validate_resume_checks_assumption_and_fingerprints() {
        let stamp = ResultProviderId::POLYNOMIAL.stamped();
        let mut frontier = ComputationFrontier::new(0x11, ResumeToken::empty_with_provider(ResumeKind::Cut, stamp));
        frontier.plan_fingerprint = Some(0x22);
        frontier.object_fingerprints = vec![7, 8];
        frontier.assumption_scope = Some(AssumptionSetId(3));

        assert!(frontier
            .validate_resume(check(stamp, Some(AssumptionSetId(3)), 0x11, Some(0x22), &[7, 8], &[], false, None))
            .is_ok());

        let scope_err = frontier
            .validate_resume(check(stamp, Some(AssumptionSetId(4)), 0x11, Some(0x22), &[7, 8], &[], false, None))
            .expect_err("scope");
        assert_eq!(scope_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("assumption_scope_changed"));

        let goal_err = frontier
            .validate_resume(check(stamp, Some(AssumptionSetId(3)), 0x99, Some(0x22), &[7, 8], &[], false, None))
            .expect_err("goal");
        assert_eq!(goal_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("goal_fingerprint_mismatch"));

        let objects_err = frontier
            .validate_resume(check(stamp, Some(AssumptionSetId(3)), 0x11, Some(0x22), &[7], &[], false, None))
            .expect_err("objects");
        assert_eq!(
            objects_err.details.get("reason").map(|v| v.to_string()).as_deref(),
            Some("object_fingerprints_mismatch")
        );
    }

    #[test]
    fn validate_resume_rejects_missing_certificate_and_budget() {
        let stamp = ResultProviderId::NUMBER_THEORY.stamped();
        let mut frontier = ComputationFrontier::new(1, ResumeToken::empty_with_provider(ResumeKind::Cut, stamp));
        frontier.certificate_fingerprints = vec![100, 200];
        frontier.budget_consumed = 5;

        assert!(frontier
            .validate_resume(check(stamp, None, 1, None, &[], &[200, 100, 300], false, Some(10)))
            .is_ok());

        let cert_err = frontier
            .validate_resume(check(stamp, None, 1, None, &[], &[100], false, Some(10)))
            .expect_err("cert");
        assert_eq!(
            cert_err.details.get("reason").map(|v| v.to_string()).as_deref(),
            Some("certificate_not_replayable")
        );

        let cancel_err = frontier
            .validate_resume(check(stamp, None, 1, None, &[], &[100, 200], true, Some(10)))
            .expect_err("cancel");
        assert_eq!(cancel_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("cancelled"));

        let budget_err = frontier
            .validate_resume(check(stamp, None, 1, None, &[], &[100, 200], false, Some(5)))
            .expect_err("budget");
        assert_eq!(budget_err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("budget_exhausted"));
    }
}
