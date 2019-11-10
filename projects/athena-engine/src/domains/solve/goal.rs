//! Solve 目标族。
//!
//! 不同前端入口 lowering 到不同 goal；禁止压成同一返回类型。
//! 方言表面函数名只存在于 SXO adapter / 文档，不写入本枚举合同。

/// 求解目标（完整性承诺随 goal 变化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolveGoal {
    /// 完整精确解集。结果必须区分条件、覆盖率与完备性。
    ExactSolutionSet,
    /// 数值近似根集（不承诺符号完备）。
    NumericalRootSet,
    /// 与原问题等价的条件 / 半代数描述。
    ConstraintDescription,
    /// 量词消去，产出无量词公式。
    QuantifierElimination,
    /// 指定域上的量词真假判定。
    QuantifierDecision,
    /// 消元理想 / 投影关系。
    EliminationIdeal,
    /// 模型查找（存在性实例，不承诺完整解集）。
    ModelFinding,
    /// 局部数值根（初值邻域）。
    LocalNumericalRoot,
    /// 线性系统精确 / 结构化求解。
    LinearSystemSolve,
    /// 多项式根集（一元或指定表示）。
    PolynomialRootSet,
    /// 微分方程解。
    DifferentialSolution,
    /// 递推关系解。
    RecurrenceSolution,
}

impl SolveGoal {
    /// 是否本质上只承诺局部 / 实例，而非全局完整解集。
    pub fn is_inherently_local_or_partial(self) -> bool {
        matches!(self, Self::ModelFinding | Self::LocalNumericalRoot)
    }

    /// 结果是否应是条件公式而非绑定枚举。
    pub fn yields_constraint_formula(self) -> bool {
        matches!(
            self,
            Self::ConstraintDescription | Self::QuantifierElimination | Self::QuantifierDecision | Self::EliminationIdeal
        )
    }
}
