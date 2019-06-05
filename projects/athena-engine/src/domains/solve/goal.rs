//! Solve 目标族。
//!
//! 不同前端入口 lowering 到不同 goal；禁止压成同一返回类型。

/// 求解目标（完整性承诺随 goal 变化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolveGoal {
    /// 精确解集（Mathematica `Solve` / MATLAB `solve`）。
    ExactSolutionSet,
    /// 数值根集（`NSolve` / `vpasolve`）。
    NumericalRootSet,
    /// 与原问题等价的条件描述（`Reduce` 一侧）。
    ConstraintDescription,
    /// 量词消去（`Reduce`/`Resolve` 量化侧）。
    QuantifierElimination,
    /// 指定域上的量词判定（`Resolve`）。
    QuantifierDecision,
    /// 消元理想 / 投影关系（`Eliminate`）。
    EliminationIdeal,
    /// 模型查找（`FindInstance`，不承诺完整）。
    ModelFinding,
    /// 局部数值根（`FindRoot` / `fsolve`）。
    LocalNumericalRoot,
    /// 线性系统（`LinearSolve` / `linsolve` / `A\b`）。
    LinearSystemSolve,
    /// 多项式根集（MATLAB `roots` 等）。
    PolynomialRootSet,
    /// 微分方程解（`DSolve` / `dsolve`）。
    DifferentialSolution,
    /// 递推解（`RSolve`）。
    RecurrenceSolution,
}

impl SolveGoal {
    /// 是否本质上只承诺局部 / 实例，而非全局完整解集。
    pub fn is_inherently_local_or_partial(self) -> bool {
        matches!(self, Self::ModelFinding | Self::LocalNumericalRoot)
    }

    /// 结果是否应是条件公式而非绑定枚举。
    pub fn yields_constraint_formula(self) -> bool {
        matches!(self, Self::ConstraintDescription | Self::QuantifierElimination | Self::QuantifierDecision | Self::EliminationIdeal)
    }
}
