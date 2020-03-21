//! 算法保证级别与线性方程组求解状态。

/// 结果保证级别（所有 det/rank/solve 必须携带）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlgorithmGuarantee {
    /// 精确可验证。
    Exact,
    /// 概率性（随机化路径；精确/机器路径不产出）。
    Probable,
    /// 机器近似。
    Approximate,
    /// 部分结果。
    Partial,
    /// 当前不支持。
    Unsupported,
}

/// 线性方程组求解分类。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
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

impl SolveDisposition {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Unique => Self::Unique,
            Self::Infinite { free_vars } => Self::Infinite {
                free_vars: free_vars.clone(),
            },
            Self::Inconsistent => Self::Inconsistent,
            Self::Singular => Self::Singular,
            Self::ResourceLimited => Self::ResourceLimited,
        }
    }
}

/// 数值残差与条件信息（机器路径）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachineSolveWitness {
    /// `‖Ax − b‖_∞`。
    pub residual_inf: f64,
    /// 估计数值秩。
    pub numerical_rank: u64,
    /// 主元阈值（用于秩判定）。
    pub pivot_threshold: f64,
}
