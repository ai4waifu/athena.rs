//! Solve 域请求（Session arena 方程 → 统一解合同）。

use athena_types::{SymbolId, TermId};

/// 顶层 Solve 请求（由方言 Goal 降低）。
#[derive(Debug, PartialEq)]
pub enum SolveRequest {
    /// 一元方程 `Equal[lhs, rhs]`（或已是方程约束）对单个未知量求精确根集。
    UnivariateEquation {
        /// 方程项（通常为 `Equal[…]`）。
        equation: TermId,
        /// 未知量。
        unknown: SymbolId,
    },
    /// 仿射线性方程组对多个未知量求精确解集（投影为 Rules）。
    LinearEquations {
        /// 方程项列表（每项通常为 `Equal[…]`）。
        equations: Vec<TermId>,
        /// 未知量（列顺序即矩阵列顺序）。
        unknowns: Vec<SymbolId>,
    },
}

impl SolveRequest {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::UnivariateEquation { equation, unknown } => Self::UnivariateEquation { equation: *equation, unknown: *unknown },
            Self::LinearEquations { equations, unknowns } => Self::LinearEquations {
                equations: equations.clone(),
                unknowns: unknowns.clone(),
            },
        }
    }
}
