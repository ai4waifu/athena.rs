//! 结构化诊断 — 语言中立。

/// 严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 硬失败。
    Error,
    /// 警告。
    Warning,
}

/// 稳定的 athena 诊断码（`athena_*`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// 非法数字字面量。
    InvalidNumber,
    /// 域错误。
    DomainError,
    /// 除以零。
    DivideByZero,
    /// 类型不匹配。
    TypeMismatch,
    /// 形状不匹配。
    ShapeMismatch,
    /// 未绑定符号。
    UnboundSymbol,
    /// 未知算子。
    UnknownOperator,
    /// 不支持的操作。
    UnsupportedOperation,
    /// 非法下标。
    InvalidIndex,
    /// 禁止精度损失时发生精度损失。
    PrecisionLoss,
    /// 赋值错误。
    AssignmentError,
    /// 不收敛。
    NonConvergent,
    /// 数值提升失败。
    PromotionFailed,
    /// 指数越界。
    ExponentOutOfRange,
    /// 当前假设下微积分表达式无定义。
    CalculusUndefined,
    /// 在该点 / 假设下导数不存在。
    DerivativeNotExist,
    /// 极限不存在。
    LimitDoesNotExist,
    /// 极限振荡。
    LimitOscillatory,
    /// 积分发散。
    IntegralDivergent,
    /// 未找到初等原函数。
    IntegralNotElementary,
    /// 积分域非法。
    IntegrationDomainInvalid,
    /// 分支选择歧义。
    BranchAmbiguous,
    /// 所需假设未消解。
    AssumptionUnresolved,
    /// 触及级数阶数 / 余项上限。
    SeriesOrderLimit,
    /// 级数余项未知。
    SeriesRemainderUnknown,
    /// ODE 类别不支持。
    OdeUnsupported,
    /// ODE 解未通过验证。
    OdeSolutionUnverified,
    /// 变换收敛域（ROC）未知。
    TransformRocUnknown,
    /// 数值结果未认证。
    NumericNotCertified,
    /// 微积分资源 / 改写上限。
    CalculusResourceLimit,
    /// 域 / 模数绑定不匹配。
    DomainMismatch,
    /// 因式分解不完整（带结构化 metadata 时仍可返回部分结果）。
    FactorIncomplete,
    /// 素性判定未决。
    PrimeTestInconclusive,
    /// 模数非法（非 `> 1` 等）。
    ModulusInvalid,
    /// 模逆不存在（与模不互素）。
    ModularInverseMissing,
    /// 同余系统无解 / 不一致。
    CongruenceInconsistent,
    /// 多项式在非域上非法除法。
    PolynomialNonFieldDivision,
    /// 多项式除零。
    PolynomialDivisionByZero,
    /// 多项式变量集不匹配。
    PolynomialVariableMismatch,
    /// 多项式过大（资源）。
    PolynomialTooLarge,
    /// Gröbner 资源上限。
    GroebnerResourceLimit,
}

impl DiagnosticCode {
    /// Wire 字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidNumber => "athena_INVALID_NUMBER",
            Self::DomainError => "athena_DOMAIN_ERROR",
            Self::DivideByZero => "athena_DIVIDE_BY_ZERO",
            Self::TypeMismatch => "athena_TYPE_MISMATCH",
            Self::ShapeMismatch => "athena_SHAPE_MISMATCH",
            Self::UnboundSymbol => "athena_UNBOUND_SYMBOL",
            Self::UnknownOperator => "athena_UNKNOWN_OPERATOR",
            Self::UnsupportedOperation => "athena_UNSUPPORTED_OPERATION",
            Self::InvalidIndex => "athena_INVALID_INDEX",
            Self::PrecisionLoss => "athena_PRECISION_LOSS",
            Self::AssignmentError => "athena_ASSIGNMENT_ERROR",
            Self::NonConvergent => "athena_NON_CONVERGENT",
            Self::PromotionFailed => "athena_PROMOTION_FAILED",
            Self::ExponentOutOfRange => "athena_EXPONENT_OUT_OF_RANGE",
            Self::CalculusUndefined => "athena_CALCULUS_UNDEFINED",
            Self::DerivativeNotExist => "athena_DERIVATIVE_NOT_EXIST",
            Self::LimitDoesNotExist => "athena_LIMIT_DOES_NOT_EXIST",
            Self::LimitOscillatory => "athena_LIMIT_OSCILLATORY",
            Self::IntegralDivergent => "athena_INTEGRAL_DIVERGENT",
            Self::IntegralNotElementary => "athena_INTEGRAL_NOT_ELEMENTARY",
            Self::IntegrationDomainInvalid => "athena_INTEGRATION_DOMAIN_INVALID",
            Self::BranchAmbiguous => "athena_BRANCH_AMBIGUOUS",
            Self::AssumptionUnresolved => "athena_ASSUMPTION_UNRESOLVED",
            Self::SeriesOrderLimit => "athena_SERIES_ORDER_LIMIT",
            Self::SeriesRemainderUnknown => "athena_SERIES_REMAINDER_UNKNOWN",
            Self::OdeUnsupported => "athena_ODE_UNSUPPORTED",
            Self::OdeSolutionUnverified => "athena_ODE_SOLUTION_UNVERIFIED",
            Self::TransformRocUnknown => "athena_TRANSFORM_ROC_UNKNOWN",
            Self::NumericNotCertified => "athena_NUMERIC_NOT_CERTIFIED",
            Self::CalculusResourceLimit => "athena_CALCULUS_RESOURCE_LIMIT",
            Self::DomainMismatch => "athena_DOMAIN_MISMATCH",
            Self::FactorIncomplete => "athena_FACTOR_INCOMPLETE",
            Self::PrimeTestInconclusive => "athena_PRIME_TEST_INCONCLUSIVE",
            Self::ModulusInvalid => "athena_MODULUS_INVALID",
            Self::ModularInverseMissing => "athena_MODULAR_INVERSE_MISSING",
            Self::CongruenceInconsistent => "athena_CONGRUENCE_INCONSISTENT",
            Self::PolynomialNonFieldDivision => "athena_POLYNOMIAL_NON_FIELD_DIVISION",
            Self::PolynomialDivisionByZero => "athena_POLYNOMIAL_DIVISION_BY_ZERO",
            Self::PolynomialVariableMismatch => "athena_POLYNOMIAL_VARIABLE_MISMATCH",
            Self::PolynomialTooLarge => "athena_POLYNOMIAL_TOO_LARGE",
            Self::GroebnerResourceLimit => "athena_GROEBNER_RESOURCE_LIMIT",
        }
    }
}

/// 结构化诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// 诊断码。
    pub code: DiagnosticCode,
    /// 严重级别。
    pub severity: Severity,
    /// 中立细节（不本地化）。
    pub detail: String,
}

impl Diagnostic {
    /// 错误级诊断。
    pub fn error(code: DiagnosticCode, detail: impl Into<String>) -> Self {
        Self { code, severity: Severity::Error, detail: detail.into() }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for Diagnostic {}

/// Result 别名。
pub type Result<T> = std::result::Result<T, Diagnostic>;
