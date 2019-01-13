//! 微积分域请求（面向宿主的稳定 wire 形态）。

use athena_types::AssumptionSet;

use crate::term::Term;

/// 求导阶数。
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq)]
pub enum LimitApproach {
    /// 有限点（已解码项，非源码文本）。
    Finite(Term),
    /// +∞。
    PositiveInfinity,
    /// −∞。
    NegativeInfinity,
}

/// 实极限的侧向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformKind {
    /// 单边 Laplace 变换。
    Laplace,
    /// Fourier 变换。
    Fourier,
    /// Z 变换。
    Z,
}

/// 微积分域请求 — 宿主将方言形态映射至此。
#[derive(Debug, Clone, PartialEq)]
pub enum CalculusRequest {
    /// 常导数 / 高阶导数。
    Derivative {
        /// 表达式（已解码）。
        expression: Term,
        /// 求导变量名（在 SymbolId 绑定落地前的桥接）。
        variable: String,
        /// 阶数。
        order: DerivativeOrder,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 极限。
    Limit {
        /// 表达式。
        expression: Term,
        /// 变量。
        variable: String,
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
        expression: Term,
        /// 积分变量。
        variable: String,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 有限区间上的定积分。
    DefiniteIntegral {
        /// 表达式。
        expression: Term,
        /// 积分变量。
        variable: String,
        /// 下限（已解码）。
        lower: Term,
        /// 上限（已解码）。
        upper: Term,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 关于展开中心的 Taylor / 幂级数。
    Series {
        /// 表达式。
        expression: Term,
        /// 展开变量。
        variable: String,
        /// 展开中心（已解码）。
        center: Term,
        /// 包含的最高幂次。
        order: u32,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 关于展开中心的 Laurent 级数（允许负幂）。
    Laurent {
        /// 表达式。
        expression: Term,
        /// 展开变量。
        variable: String,
        /// 展开中心（已解码）。
        center: Term,
        /// 正则部分包含的最高幂次。
        order: u32,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 当变量趋于 `+∞` 的渐近级数。
    Asymptotic {
        /// 表达式。
        expression: Term,
        /// 展开变量。
        variable: String,
        /// 保留的 `t=1/x` 最高幂次。
        order: u32,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 复留数 `Res(f, z→a)`。
    Residue {
        /// 被积 / 被展表达式。
        expression: Term,
        /// 复变量。
        variable: String,
        /// 奇点 / 展开点（已解码）。
        point: Term,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 标量场的梯度。
    Gradient {
        /// 标量表达式。
        expression: Term,
        /// 按序变量。
        variables: Vec<String>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 向量值映射的 Jacobian。
    Jacobian {
        /// 分量表达式。
        expressions: Vec<Term>,
        /// 自变量。
        variables: Vec<String>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 标量场的 Hessian。
    Hessian {
        /// 标量表达式。
        expression: Term,
        /// 按序变量（混合偏导保持此顺序）。
        variables: Vec<String>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 向量场散度。
    Divergence {
        /// 分量 F₁…Fₙ。
        components: Vec<Term>,
        /// 坐标变量（与分量同序）。
        variables: Vec<String>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 三维向量场旋度。
    Curl {
        /// 分量 (Fₓ, Fᵧ, F_z)。
        components: Vec<Term>,
        /// 坐标 (x, y, z)。
        variables: Vec<String>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 一阶 ODE 求解（引导实现子集）。
    SolveOde {
        /// 方程项（`Equal[…]`）。
        equation: Term,
        /// 因变量。
        dependent: String,
        /// 自变量。
        independent: String,
        /// 可选初值问题 `(x0, y0)`（已解码）。
        initial: Option<(Term, Term)>,
        /// 假设。
        assumptions: AssumptionSet,
    },
    /// 积分变换（Laplace / Fourier / Z）。
    Transform {
        /// 种类。
        kind: TransformKind,
        /// 时域表达式。
        expression: Term,
        /// 时间 / 序列变量。
        time_variable: String,
        /// 变换变量。
        transform_variable: String,
        /// 假设。
        assumptions: AssumptionSet,
    },
}
