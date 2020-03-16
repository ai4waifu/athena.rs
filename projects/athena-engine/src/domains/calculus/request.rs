//! 微积分域请求（typed Goal wire · Living `27`/`28`）。
//!
//! 变量身份为 [`SymbolId`]。算法层若仍吃 `&str`，仅由 `execute_calculus` 解析显示名，不得反向猜 Goal。

use athena_types::{AssumptionSet, SymbolId, TermId};

/// 求导阶数。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DerivativeOrder {
    /// 一阶导数。
    First,
    /// 高阶常导数。
    Repeated(u32),
}

impl Default for DerivativeOrder {
    fn default() -> Self {
        Self::First
    }
}

/// 极限趋近方式。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum LimitApproach {
    /// 有限点（已解码项，非源码文本）。
    Finite(TermId),
    /// +∞。
    PositiveInfinity,
    /// −∞。
    NegativeInfinity,
}

impl LimitApproach {
    /// Owning 复制（Living `31`：仅 `TermId` 句柄）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Finite(t) => Self::Finite(*t),
            Self::PositiveInfinity => Self::PositiveInfinity,
            Self::NegativeInfinity => Self::NegativeInfinity,
        }
    }
}

/// 实极限的侧向。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum LimitDirection {
    /// 双侧。
    #[default]
    TwoSided,
    /// 左极限。
    FromBelow,
    /// 右极限。
    FromAbove,
}

/// 要计算的积分变换种类。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TransformKind {
    /// 单边 Laplace 变换。
    Laplace,
    /// Fourier 变换。
    Fourier,
    /// Z 变换。
    Z,
}

/// 微积分域请求 — 宿主将方言形态映射至此。变量身份为 SymbolId（Living 27/28）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum CalculusRequest {
    /// 常导数 / 高阶导数。
    Derivative {
        /// 表达式（已解码）。
        expression: TermId,
        /// 求导变量。
        variable: SymbolId,
        /// 阶数。
        order: DerivativeOrder,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 极限。
    Limit {
        /// 表达式。
        expression: TermId,
        /// 变量。
        variable: SymbolId,
        /// 趋近点。
        approach: LimitApproach,
        /// 侧向。
        direction: LimitDirection,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 不定积分。
    Integral {
        /// 表达式。
        expression: TermId,
        /// 积分变量。
        variable: SymbolId,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 有限区间上的定积分。
    DefiniteIntegral {
        /// 表达式。
        expression: TermId,
        /// 积分变量。
        variable: SymbolId,
        /// 下限（已解码）。
        lower: TermId,
        /// 上限（已解码）。
        upper: TermId,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 关于展开中心的 Taylor / 幂级数。
    Series {
        /// 表达式。
        expression: TermId,
        /// 展开变量。
        variable: SymbolId,
        /// 展开中心（已解码）。
        center: TermId,
        /// 包含的最高幂次。
        order: u32,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 关于展开中心的 Laurent 级数（允许负幂）。
    Laurent {
        /// 表达式。
        expression: TermId,
        /// 展开变量。
        variable: SymbolId,
        /// 展开中心（已解码）。
        center: TermId,
        /// 正则部分包含的最高幂次。
        order: u32,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 当变量趋于 `+∞` 的渐近级数。
    Asymptotic {
        /// 表达式。
        expression: TermId,
        /// 展开变量。
        variable: SymbolId,
        /// 保留的 `t=1/x` 最高幂次。
        order: u32,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 复留数 `Res(f, z→a)`。
    Residue {
        /// 被积 / 被展表达式。
        expression: TermId,
        /// 复变量。
        variable: SymbolId,
        /// 奇点 / 展开点（已解码）。
        point: TermId,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 标量场的梯度。
    Gradient {
        /// 标量表达式。
        expression: TermId,
        /// 按序变量。
        variables: Vec<SymbolId>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 向量值映射的 Jacobian。
    Jacobian {
        /// 分量表达式。
        expressions: Vec<TermId>,
        /// 自变量。
        variables: Vec<SymbolId>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 标量场的 Hessian。
    Hessian {
        /// 标量表达式。
        expression: TermId,
        /// 按序变量（混合偏导保持此顺序）。
        variables: Vec<SymbolId>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 向量场散度。
    Divergence {
        /// 分量 F₁…Fₙ。
        components: Vec<TermId>,
        /// 坐标变量（与分量同序）。
        variables: Vec<SymbolId>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 三维向量场旋度。
    Curl {
        /// 分量 (Fₓ, Fᵧ, F_z)。
        components: Vec<TermId>,
        /// 坐标 (x, y, z)。
        variables: Vec<SymbolId>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 一阶 ODE 求解（引导实现子集）。
    SolveOde {
        /// 方程项（`Equal[…]`）。
        equation: TermId,
        /// 因变量。
        dependent: SymbolId,
        /// 自变量。
        independent: SymbolId,
        /// 可选初值问题 `(x0, y0)`（已解码）。
        initial: Option<(TermId, TermId)>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 积分变换（Laplace / Fourier / Z）。
    Transform {
        /// 种类。
        kind: TransformKind,
        /// 时域表达式。
        expression: TermId,
        /// 时间 / 序列变量。
        time_variable: SymbolId,
        /// 变换变量。
        transform_variable: SymbolId,
        /// 假设。
        assumptions: AssumptionSet,
    },
}

impl CalculusRequest {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Derivative { expression, variable, order, assumptions } => Self::Derivative {
                expression: *expression,
                variable: *variable,
                order: *order,
                assumptions: assumptions.clone(),
            },
            Self::Limit { expression, variable, approach, direction, assumptions } => Self::Limit {
                expression: *expression,
                variable: *variable,
                approach: approach.owning_copy(),
                direction: *direction,
                assumptions: assumptions.clone(),
            },
            Self::Integral { expression, variable, assumptions } => Self::Integral {
                expression: *expression,
                variable: *variable,
                assumptions: assumptions.clone(),
            },
            Self::DefiniteIntegral { expression, variable, lower, upper, assumptions } => Self::DefiniteIntegral {
                expression: *expression,
                variable: *variable,
                lower: *lower,
                upper: *upper,
                assumptions: assumptions.clone(),
            },
            Self::Series { expression, variable, center, order, assumptions } => Self::Series {
                expression: *expression,
                variable: *variable,
                center: *center,
                order: *order,
                assumptions: assumptions.clone(),
            },
            Self::Laurent { expression, variable, center, order, assumptions } => Self::Laurent {
                expression: *expression,
                variable: *variable,
                center: *center,
                order: *order,
                assumptions: assumptions.clone(),
            },
            Self::Asymptotic { expression, variable, order, assumptions } => Self::Asymptotic {
                expression: *expression,
                variable: *variable,
                order: *order,
                assumptions: assumptions.clone(),
            },
            Self::Residue { expression, variable, point, assumptions } => Self::Residue {
                expression: *expression,
                variable: *variable,
                point: *point,
                assumptions: assumptions.clone(),
            },
            Self::Gradient { expression, variables, assumptions } => Self::Gradient {
                expression: *expression,
                variables: variables.clone(),
                assumptions: assumptions.clone(),
            },
            Self::Jacobian { expressions, variables, assumptions } => Self::Jacobian {
                expressions: expressions.clone(),
                variables: variables.clone(),
                assumptions: assumptions.clone(),
            },
            Self::Hessian { expression, variables, assumptions } => Self::Hessian {
                expression: *expression,
                variables: variables.clone(),
                assumptions: assumptions.clone(),
            },
            Self::Divergence { components, variables, assumptions } => Self::Divergence {
                components: components.clone(),
                variables: variables.clone(),
                assumptions: assumptions.clone(),
            },
            Self::Curl { components, variables, assumptions } => Self::Curl {
                components: components.clone(),
                variables: variables.clone(),
                assumptions: assumptions.clone(),
            },
            Self::SolveOde { equation, dependent, independent, initial, assumptions } => Self::SolveOde {
                equation: *equation,
                dependent: *dependent,
                independent: *independent,
                initial: *initial,
                assumptions: assumptions.clone(),
            },
            Self::Transform { kind, expression, time_variable, transform_variable, assumptions } => Self::Transform {
                kind: *kind,
                expression: *expression,
                time_variable: *time_variable,
                transform_variable: *transform_variable,
                assumptions: assumptions.clone(),
            },
        }
    }
}
