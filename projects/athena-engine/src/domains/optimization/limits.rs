//! 算法策略与资源上限。

/// 优化资源与精度上限。
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct OptimizationLimits {
    /// 最大变量数。
    pub max_variables: Option<u32>,
    /// 最大约束数。
    pub max_constraints: Option<u32>,
    /// 最大迭代。
    pub max_iterations: Option<u64>,
    /// 最大 branch-and-bound 节点。
    pub max_nodes: Option<u64>,
    /// 时间预算（毫秒）。
    pub time_budget_ms: Option<u64>,
    /// 内存预算（字节）。
    pub memory_budget_bytes: Option<u64>,
    /// 相对最优性 gap 容差。
    pub gap_tolerance: Option<f64>,
    /// 绝对 gap 容差。
    pub absolute_gap_tolerance: Option<f64>,
    /// 数值可行性残差容差。
    pub feasibility_tolerance: Option<f64>,
}

impl Default for OptimizationLimits {
    fn default() -> Self {
        Self {
            max_variables: None,
            max_constraints: None,
            max_iterations: None,
            max_nodes: None,
            time_budget_ms: None,
            memory_budget_bytes: None,
            gap_tolerance: None,
            absolute_gap_tolerance: None,
            feasibility_tolerance: None,
        }
    }
}
