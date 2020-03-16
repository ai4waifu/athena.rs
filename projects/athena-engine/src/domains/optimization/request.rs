//! 优化域请求。

use super::{frontier::OptimizationFrontier, problem::OptimizationProblem};

/// 优化请求变体。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum OptimizationRequest {
    /// 校验问题合同（整数性、域、空目标等）。
    ValidateProblem {
        /// 问题。
        problem: OptimizationProblem,
    },
    /// 求解（算法策略由问题 `policy` 指定）。
    Solve {
        /// 问题。
        problem: OptimizationProblem,
    },
    /// 独立验证证书（载荷后续接入）。
    VerifyCertificate {
        /// 问题。
        problem: OptimizationProblem,
    },
    /// 从 frontier 恢复。
    Resume {
        /// 问题。
        problem: OptimizationProblem,
        /// 前沿。
        frontier: OptimizationFrontier,
    },
}

impl OptimizationRequest {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::ValidateProblem { problem } => Self::ValidateProblem { problem: problem.owning_copy() },
            Self::Solve { problem } => Self::Solve { problem: problem.owning_copy() },
            Self::VerifyCertificate { problem } => Self::VerifyCertificate { problem: problem.owning_copy() },
            Self::Resume { problem, frontier } => Self::Resume {
                problem: problem.owning_copy(),
                frontier: frontier.owning_copy(),
            },
        }
    }
}
