//! 算法保证级别与线性方程组求解状态。

/// 结果保证级别（所有 det/rank/solve 必须携带）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlgorithmGuarantee {
    /// 精确可验证。
    Exact,
    /// 概率性（L4 随机化；L1 不产出）。
    Probable,
    /// 机器近似。
    Approximate,
    /// 部分结果。
    Partial,
    /// 当前不支持。
    Unsupported,
}

/// 线性方程组求解分类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveDisposition {
    /// 唯一解。
    Unique,
    /// 无穷多解（自由变量列下标，0-based）。
    Infinite {
        /// 自由变量列。
        free_vars: Vec<u64>,
    },
    /// 无解。
    Inconsistent,
    /// 奇异 / 秩亏导致无法按请求形态求解。
    Singular,
    /// 资源限制。
    ResourceLimited,
}

/// 数值残差与条件信息（机器路径）。
#[derive(Debug, Clone, PartialEq)]
pub struct MachineSolveWitness {
    /// `‖Ax − b‖_∞`。
    pub residual_inf: f64,
    /// 估计数值秩。
    pub numerical_rank: u64,
    /// 主元阈值（用于秩判定）。
    pub pivot_threshold: f64,
}
