//! Structured diagnostics — language-neutral.

/// Severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Hard failure.
    Error,
    /// Warning.
    Warning,
}

/// Stable athena diagnostic code (`athena_*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// Invalid numeric literal.
    InvalidNumber,
    /// Domain error.
    DomainError,
    /// Division by zero.
    DivideByZero,
    /// Type mismatch.
    TypeMismatch,
    /// Shape mismatch.
    ShapeMismatch,
    /// Unbound symbol.
    UnboundSymbol,
    /// Unknown operator.
    UnknownOperator,
    /// Unsupported operation.
    UnsupportedOperation,
    /// Invalid index.
    InvalidIndex,
    /// Precision loss when forbidden.
    PrecisionLoss,
    /// Assignment error.
    AssignmentError,
    /// Non-convergent.
    NonConvergent,
    /// Numeric promotion failed.
    PromotionFailed,
    /// Exponent out of range.
    ExponentOutOfRange,
    /// Calculus expression undefined under current assumptions.
    CalculusUndefined,
    /// Derivative does not exist at the point / under assumptions.
    DerivativeNotExist,
    /// Limit does not exist.
    LimitDoesNotExist,
    /// Limit is oscillatory.
    LimitOscillatory,
    /// Integral diverges.
    IntegralDivergent,
    /// No elementary antiderivative found.
    IntegralNotElementary,
    /// Integration domain invalid.
    IntegrationDomainInvalid,
    /// Branch choice ambiguous.
    BranchAmbiguous,
    /// Required assumption unresolved.
    AssumptionUnresolved,
    /// Series order / remainder limit hit.
    SeriesOrderLimit,
    /// Series remainder unknown.
    SeriesRemainderUnknown,
    /// ODE class unsupported.
    OdeUnsupported,
    /// ODE solution failed verification.
    OdeSolutionUnverified,
    /// Transform region of convergence unknown.
    TransformRocUnknown,
    /// Numeric result is not certified.
    NumericNotCertified,
    /// Calculus resource / rewrite limit.
    CalculusResourceLimit,
}

impl DiagnosticCode {
    /// Wire string.
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
        }
    }
}

/// Structured diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Code.
    pub code: DiagnosticCode,
    /// Severity.
    pub severity: Severity,
    /// Neutral detail (not localized).
    pub detail: String,
}

impl Diagnostic {
    /// Error diagnostic.
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

/// Result alias.
pub type Result<T> = std::result::Result<T, Diagnostic>;
