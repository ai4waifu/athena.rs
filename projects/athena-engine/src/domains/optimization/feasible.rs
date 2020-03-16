//! 可行集。

use athena_types::DomainId;

use super::constraint::Constraint;

/// 可行域闭包 / 规范化状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClosureStatus {
    /// 尚未规范化。
    Open,
    /// 已规范化（仍可能未判定可行性）。
    Normalized,
    /// 已证明为空（不可行）。
    ProvenEmpty,
    /// 资源截断下的部分闭包。
    ResourceLimited,
    /// 未知。
    Unknown,
}

/// 可行集：约束 + 域 + 闭包状态。
///
/// 可行性 ≠ 最优性。本对象不携带目标值。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct FeasibleSet {
    /// 约束列表。
    pub constraints: Vec<Constraint>,
    /// 共同域。
    pub domain: DomainId,
    /// 闭包状态。
    pub closure_status: ClosureStatus,
}

impl FeasibleSet {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            constraints: self.constraints.iter().map(Constraint::owning_copy).collect(),
            domain: self.domain,
            closure_status: self.closure_status,
        }
    }

    /// 空约束集（尚未判定）。
    pub fn empty(domain: DomainId) -> Self {
        Self { constraints: Vec::new(), domain, closure_status: ClosureStatus::Open }
    }
}
