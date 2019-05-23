//! 求解请求（调度协议，非 Solve 数学对象）。
//!
//! [`SolverRequest`] 只描述「向某个 provider 发出的执行请求」。
//! 问题语义、unknown/parameter、goal、coverage 属于 [`crate::domains::solve::SolveProblem`]。

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
    /// 线性代数。
    LinearAlgebra,
    /// 统一 Solve 流水线。
    Solve,
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

/// Provider 调度请求。
///
/// 不表达 unknown/parameter 分离、quantifier、SolutionSet coverage 或 completeness proof。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverRequest {
    /// 域。
    pub domain: DomainRef,
    /// 根项（候选输入，非完整 SolveProblem）。
    pub roots: Vec<TermId>,
    /// 操作。
    pub operation: SolverOperation,
    /// 限制。
    pub limits: SolverLimits,
    /// 假设。
    pub assumptions: AssumptionSetId,
}
