//! 求解策略与执行限制。

use athena_types::Precision;

/// 求解策略（初值、精度、停止准则等操作性状态）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SolvePolicy {
    /// 目标精度。
    pub precision: Option<Precision>,
    /// 机器可读策略标签（非用户文案）。
    pub tags: Vec<String>,
}

/// 执行资源限制（可恢复截断的预算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecutionLimits {
    /// 最大毫秒。
    pub max_millis: Option<u64>,
    /// 最大节点 / 项展开。
    pub max_nodes: Option<u32>,
    /// 最大分支数。
    pub max_branches: Option<u32>,
    /// 最大迭代次数（数值路径）。
    pub max_iterations: Option<u32>,
}
