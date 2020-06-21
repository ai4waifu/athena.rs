//! 封闭核心语义算子与应用 head。
//!
//! 核心数学 / 逻辑 / 结构算子为 [`SemanticOperator`]。
//! [`ExtensionRegistry`] 仅用于扩展显示名，绝非核心目录。

use athena_types::ExtensionOperatorId;

/// 封闭一元特殊函数标识（指纹稳定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryFunction {
    /// 指数。
    Exp,
    /// 自然对数。
    Log,
    /// 正弦。
    Sin,
    /// 余弦。
    Cos,
    /// 正切。
    Tan,
    /// 双曲正弦。
    Sinh,
    /// 双曲余弦。
    Cosh,
    /// 双曲正切。
    Tanh,
    /// 反正弦。
    ArcSin,
    /// 反余弦。
    ArcCos,
    /// 反正切。
    ArcTan,
    /// 平方根。
    Sqrt,
    /// 绝对值。
    Abs,
    /// 符号函数。
    Sign,
    /// Gamma 函数。
    Gamma,
    /// 误差函数。
    Erf,
}

impl UnaryFunction {
    /// 稳定 discriminant 片段（勿轻易重编号）。
    pub const fn discriminant(self) -> u32 {
        match self {
            Self::Exp => 1,
            Self::Log => 2,
            Self::Sin => 3,
            Self::Cos => 4,
            Self::Tan => 5,
            Self::Sinh => 6,
            Self::Cosh => 7,
            Self::Tanh => 8,
            Self::ArcSin => 9,
            Self::ArcCos => 10,
            Self::ArcTan => 11,
            Self::Sqrt => 12,
            Self::Abs => 13,
            Self::Sign => 14,
            Self::Gamma => 15,
            Self::Erf => 16,
        }
    }

    /// 中立调试标签。
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::Exp => "Exp",
            Self::Log => "Log",
            Self::Sin => "Sin",
            Self::Cos => "Cos",
            Self::Tan => "Tan",
            Self::Sinh => "Sinh",
            Self::Cosh => "Cosh",
            Self::Tanh => "Tanh",
            Self::ArcSin => "ArcSin",
            Self::ArcCos => "ArcCos",
            Self::ArcTan => "ArcTan",
            Self::Sqrt => "Sqrt",
            Self::Abs => "Abs",
            Self::Sign => "Sign",
            Self::Gamma => "Gamma",
            Self::Erf => "Erf",
        }
    }

    /// 由 discriminant 片段还原（与 [`Self::discriminant`] 对偶）。
    pub const fn from_discriminant(d: u32) -> Option<Self> {
        match d {
            1 => Some(Self::Exp),
            2 => Some(Self::Log),
            3 => Some(Self::Sin),
            4 => Some(Self::Cos),
            5 => Some(Self::Tan),
            6 => Some(Self::Sinh),
            7 => Some(Self::Cosh),
            8 => Some(Self::Tanh),
            9 => Some(Self::ArcSin),
            10 => Some(Self::ArcCos),
            11 => Some(Self::ArcTan),
            12 => Some(Self::Sqrt),
            13 => Some(Self::Abs),
            14 => Some(Self::Sign),
            15 => Some(Self::Gamma),
            16 => Some(Self::Erf),
            _ => None,
        }
    }
}

/// 封闭 Athena 核心语义算子标识（指纹稳定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticOperator {
    // 算术
    /// `a + b + …`
    Add,
    /// `a - b` 或一元负号形式。
    Subtract,
    /// `a * b * …`
    Multiply,
    /// `a / b`
    Divide,
    /// `a ^ b`
    Power,
    /// 一元取负。
    Negate,
    /// 逐元素乘。
    ElementwiseMultiply,
    /// 逐元素除。
    ElementwiseDivide,
    /// 逐元素幂。
    ElementwisePower,
    // 比较 / 逻辑
    /// 结构 / 数值相等。
    Equal,
    /// 不等。
    Unequal,
    /// 同一（相同结构 / 槽位标识）。
    Identical,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessEqual,
    /// `>=`
    GreaterEqual,
    /// 布尔与。
    And,
    /// 布尔或。
    Or,
    /// 布尔非。
    Not,
    /// 真值查询。
    TrueQ,
    // structure
    /// 绝对值（亦见 [`UnaryFunction::Abs`]）。
    Abs,
    /// 集合长度。
    Length,
    /// 首元素。
    First,
    /// 集合其余部分。
    Rest,
    /// 阶乘。
    Factorial,
    /// 平方根（亦见 [`UnaryFunction::Sqrt`]）。
    Sqrt,
    /// 拼接集合。
    Join,
    /// 整数范围。
    Range,
    /// 将 head 作用于参数。
    Apply,
    /// apply-head / 应用形式包装。
    ApplyHead,
    /// 尺寸 / 维数。
    Size,
    /// 求和。
    Sum,
    /// 求积。
    Product,
    /// 矩阵行列式。
    Determinant,
    /// 对集合做 map。
    Map,
    /// 零矩阵 / 数组构造。
    Zeros,
    /// 全一阵 / 数组构造。
    Ones,
    /// 单位矩阵构造。
    Eye,
    /// 立即重写规则。
    Rule,
    /// 延迟重写规则。
    RuleDeferred,
    /// 全部替换。
    ReplaceAll,
    /// 收集模式匹配。
    CollectMatches,
    /// 匹配谓词。
    Matches,
    /// 化简。
    Simplify,
    /// 保持 / 引用参数。
    Hold,
    /// 匿名函数绑定器。
    Function,
    /// 封闭一元特殊函数。
    Unary(UnaryFunction),
    /// polygamma 残差 / 特殊函数（`PolyGamma[n, z]`）。
    PolyGamma,
    // 微积分残差 head（入口为 `DomainGoal`，非字符串 lowering）
    /// 微分残差。
    Differentiate,
    /// 积分残差。
    Integrate,
    /// 极限残差。
    Limit,
    /// 幂级数残差。
    Series,
    /// Laurent 级数残差。
    LaurentSeries,
    /// 渐近展开残差。
    Asymptotic,
    /// 留数残差。
    Residue,
    /// ODE 求解残差。
    DSolve,
    /// Laplace 变换残差。
    LaplaceTransform,
    /// Fourier 变换残差。
    FourierTransform,
    /// Z 变换残差。
    ZTransform,
    /// 向量散度残差。
    Divergence,
    /// 向量旋度残差。
    Curl,
}

impl SemanticOperator {
    /// 指纹用的稳定 discriminant（勿轻易重编号）。
    pub const fn discriminant(self) -> u32 {
        match self {
            Self::Add => 1,
            Self::Subtract => 2,
            Self::Multiply => 3,
            Self::Divide => 4,
            Self::Power => 5,
            Self::Negate => 6,
            Self::ElementwiseMultiply => 7,
            Self::ElementwiseDivide => 8,
            Self::ElementwisePower => 9,
            Self::Equal => 10,
            Self::Unequal => 11,
            Self::Identical => 12,
            Self::Less => 13,
            Self::Greater => 14,
            Self::LessEqual => 15,
            Self::GreaterEqual => 16,
            Self::And => 17,
            Self::Or => 18,
            Self::Not => 19,
            Self::TrueQ => 20,
            Self::Abs => 21,
            Self::Length => 22,
            Self::First => 23,
            Self::Rest => 24,
            Self::Factorial => 25,
            Self::Sqrt => 26,
            Self::Join => 27,
            Self::Range => 28,
            Self::Apply => 29,
            Self::ApplyHead => 30,
            Self::Size => 31,
            Self::Sum => 32,
            Self::Product => 33,
            Self::Determinant => 34,
            Self::Map => 35,
            Self::Zeros => 36,
            Self::Ones => 37,
            Self::Eye => 38,
            Self::Rule => 39,
            Self::RuleDeferred => 40,
            Self::ReplaceAll => 41,
            Self::CollectMatches => 42,
            Self::Matches => 43,
            Self::Simplify => 44,
            Self::Hold => 45,
            Self::Function => 46,
            Self::Unary(f) => 100 + f.discriminant(),
            Self::PolyGamma => 200,
            Self::Differentiate => 201,
            Self::Integrate => 202,
            Self::Limit => 203,
            Self::Series => 204,
            Self::LaurentSeries => 205,
            Self::Asymptotic => 206,
            Self::Residue => 207,
            Self::DSolve => 208,
            Self::LaplaceTransform => 209,
            Self::FourierTransform => 210,
            Self::ZTransform => 211,
            Self::Divergence => 212,
            Self::Curl => 213,
        }
    }

    /// 中立调试标签（`Add`，不是方言 `Plus`）。
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Divide => "Divide",
            Self::Power => "Power",
            Self::Negate => "Negate",
            Self::ElementwiseMultiply => "ElementwiseMultiply",
            Self::ElementwiseDivide => "ElementwiseDivide",
            Self::ElementwisePower => "ElementwisePower",
            Self::Equal => "Equal",
            Self::Unequal => "Unequal",
            Self::Identical => "Identical",
            Self::Less => "Less",
            Self::Greater => "Greater",
            Self::LessEqual => "LessEqual",
            Self::GreaterEqual => "GreaterEqual",
            Self::And => "And",
            Self::Or => "Or",
            Self::Not => "Not",
            Self::TrueQ => "TrueQ",
            Self::Abs => "Abs",
            Self::Length => "Length",
            Self::First => "First",
            Self::Rest => "Rest",
            Self::Factorial => "Factorial",
            Self::Sqrt => "Sqrt",
            Self::Join => "Join",
            Self::Range => "Range",
            Self::Apply => "Apply",
            Self::ApplyHead => "ApplyHead",
            Self::Size => "Size",
            Self::Sum => "Sum",
            Self::Product => "Product",
            Self::Determinant => "Determinant",
            Self::Map => "Map",
            Self::Zeros => "Zeros",
            Self::Ones => "Ones",
            Self::Eye => "Eye",
            Self::Rule => "Rule",
            Self::RuleDeferred => "RuleDeferred",
            Self::ReplaceAll => "ReplaceAll",
            Self::CollectMatches => "CollectMatches",
            Self::Matches => "Matches",
            Self::Simplify => "Simplify",
            Self::Hold => "Hold",
            Self::Function => "Function",
            Self::Unary(f) => f.debug_label(),
            Self::PolyGamma => "PolyGamma",
            Self::Differentiate => "Differentiate",
            Self::Integrate => "Integrate",
            Self::Limit => "Limit",
            Self::Series => "Series",
            Self::LaurentSeries => "LaurentSeries",
            Self::Asymptotic => "Asymptotic",
            Self::Residue => "Residue",
            Self::DSolve => "DSolve",
            Self::LaplaceTransform => "LaplaceTransform",
            Self::FourierTransform => "FourierTransform",
            Self::ZTransform => "ZTransform",
            Self::Divergence => "Divergence",
            Self::Curl => "Curl",
        }
    }

    /// 将一元函数映射为构造 term 时优先使用的 head。
    ///
    /// `Abs` / `Sqrt` 保留专用变体，以保持结构求值对等。
    pub const fn from_unary(f: UnaryFunction) -> Self {
        match f {
            UnaryFunction::Abs => Self::Abs,
            UnaryFunction::Sqrt => Self::Sqrt,
            other => Self::Unary(other),
        }
    }

    /// 若本算子为已注册一元特殊函数（含 Abs/Sqrt 别名）。
    pub const fn as_unary(self) -> Option<UnaryFunction> {
        match self {
            Self::Unary(f) => Some(f),
            Self::Abs => Some(UnaryFunction::Abs),
            Self::Sqrt => Some(UnaryFunction::Sqrt),
            _ => None,
        }
    }
}

/// 应用 head：封闭语义算子或仅扩展标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationHead {
    /// 核心封闭语义算子。
    Semantic(SemanticOperator),
    /// 仅扩展标识。显示名可存于 [`super::ExtensionRegistry`]。
    Extension(ExtensionOperatorId),
}
