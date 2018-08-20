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
