//! athena ecosystem base types — contracts only, no eval/parser/IO.

#![deny(missing_docs)]

mod assumption;
mod diagnostic;
mod ids;
mod number;

pub use assumption::{AssumptionSet, Condition, Predicate};
pub use diagnostic::{Diagnostic, DiagnosticCode, Result, Severity};
pub use ids::{AssumptionSetId, DomainId, NodeId, OperatorId, SerializationVersion, SourceSpan, SymbolId, TermId};
pub use number::{ExactNumber, Number, RealNumber, normalize_rational};

/// Numeric scalar domain identifier (wire-stable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericDomain {
    /// Exact integers.
    Integer,
    /// Exact rationals.
    Rational,
    /// Inexact reals.
    Real,
    /// Complex numbers.
    Complex,
}

/// Rounding mode for approximate arithmetic (contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingMode {
    /// Round to nearest, ties to even.
    #[default]
    Nearest,
    /// Toward zero.
    Truncate,
    /// Toward +∞.
    Ceiling,
    /// Toward -∞.
    Floor,
}

/// Precision policy (contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// Exact arithmetic.
    Exact,
    /// IEEE binary64.
    Machine,
    /// Arbitrary bits.
    ArbitraryBits(u32),
}
