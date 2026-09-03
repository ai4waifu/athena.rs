//! Solve 相关 M-Graph 关系种类（不得混为 Equality）。

use athena_types::TermId;

use super::policy::SolvePolicy;
use crate::numeric_clone::clone_number;

/// Solve 结果可产生的关系种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveRelationKind {
    /// 候选满足问题（非完整性）。
    Satisfies {
        /// 解项 / 分支根。
        solution: TermId,
        /// 问题指纹根或问题句柄项。
        problem: TermId,
    },
    /// 解集在指定问题下完整。
    CompleteFor {
        /// 解集根。
        solution_set: TermId,
        /// 问题根。
        problem: TermId,
    },
    /// 约束等价（`Reduce` / 量词消去）。
    EquivalentConstraint {
        /// 原约束。
        left: TermId,
        /// 消去/化简后约束。
        right: TermId,
    },
    /// 代数根身份。
    RootOf {
        /// 多项式。
        polynomial: TermId,
        /// 根。
        root: TermId,
    },
    /// 唯一解。
    UniqueSolution {
        /// 问题。
        problem: TermId,
        /// 分支。
        branch: TermId,
    },
    /// 无解（需要不可满足性证书，不能由搜索失败产生）。
    NoSolution {
        /// 问题。
        problem: TermId,
    },
    /// 局部收敛（绑定 policy，非全局存在性）。
    LocalConvergence {
        /// 根。
        root: TermId,
        /// 策略摘要标签。
        policy_tag: String,
    },
}

impl SolveRelationKind {
    /// 机器标识（审计 / registry）。
    pub fn name(&self) -> &'static str {
        match self {
            Self::Satisfies { .. } => "Satisfies",
            Self::CompleteFor { .. } => "CompleteFor",
            Self::EquivalentConstraint { .. } => "EquivalentConstraint",
            Self::RootOf { .. } => "RootOf",
            Self::UniqueSolution { .. } => "UniqueSolution",
            Self::NoSolution { .. } => "NoSolution",
            Self::LocalConvergence { .. } => "LocalConvergence",
        }
    }

    /// 是否可驱动 exact rewrite / KernelIR specialization。
    pub fn drives_exact_rewrite(&self) -> bool {
        matches!(self, Self::CompleteFor { .. } | Self::EquivalentConstraint { .. } | Self::UniqueSolution { .. })
    }

    /// 由 policy 构造局部收敛关系标签。
    pub fn local_convergence(root: TermId, policy: &SolvePolicy) -> Self {
        let policy_tag = policy.tags.first().cloned().unwrap_or_else(|| "default".into());
        Self::LocalConvergence { root, policy_tag }
    }
}
