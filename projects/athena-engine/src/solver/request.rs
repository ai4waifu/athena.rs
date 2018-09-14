//! 求解请求。

use athena_types::{AssumptionSetId, TermId};

/// 领域引用（骨架字符串标签；后续换枚举 / DomainId）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainRef {
    /// 算术。
    Arithmetic,
    /// 多项式。
    Polynomial,
    /// 数论。
    NumberTheory,
    /// 群。
    Group,
    /// 域。
    Field,
    /// 伽罗瓦。
    Galois,
    /// 微积分。
    Calculus,
}

/// 求解操作标签。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverOperation {
    /// 操作名（机器标识，非用户文案）。
    pub name: String,
}

/// 求解资源限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SolverLimits {
    /// 最大毫秒。
    pub max_millis: Option<u64>,
    /// 最大节点。
    pub max_nodes: Option<u32>,
}

/// 求解请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverRequest {
    /// 域。
    pub domain: DomainRef,
    /// 根项。
    pub roots: Vec<TermId>,
    /// 操作。
    pub operation: SolverOperation,
    /// 限制。
    pub limits: SolverLimits,
    /// 假设。
    pub assumptions: AssumptionSetId,
}
