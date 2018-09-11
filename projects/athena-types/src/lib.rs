//! athena 生态基础类型 — 仅合同，无求值 / 解析 / IO。

#![deny(missing_docs)]

mod assumption;
mod diagnostic;
mod ids;
mod modulus;
mod number;

pub use assumption::{AssumptionSet, Condition, Predicate};
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticPath, DiagnosticValue, Result, Severity};
pub use ids::{
    AssumptionSetId, DomainId, ExtensionId, FieldId, GroupElementId, GroupId, NodeId, OperatorId, SerializationVersion,
    SourceSpan, SymbolId, TermId,
};
pub use modulus::{ModularValue, Modulus};
pub use number::{ExactNumber, Number, RealNumber, normalize_rational};

/// 数值标量域标识（wire 稳定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericDomain {
    /// 精确整数。
    Integer,
    /// 精确有理数。
    Rational,
    /// 非精确实数。
    Real,
    /// 复数。
    Complex,
}

/// 近似算术的舍入模式（合同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingMode {
    /// 四舍六入五成双（最近偶数）。
    #[default]
    Nearest,
    /// 向零舍入。
    Truncate,
    /// 向 +∞。
    Ceiling,
    /// 向 -∞。
    Floor,
}

/// 精度策略（合同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// 精确算术。
    Exact,
    /// IEEE binary64。
    Machine,
    /// 任意比特精度。
    ArbitraryBits(u32),
}
