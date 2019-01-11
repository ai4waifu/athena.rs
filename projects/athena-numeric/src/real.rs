//! Real numbers: IEEE binary64 or binary [`Decimal`].

use crate::decimal::Decimal;

/// Real value representation.
#[derive(Debug, Clone, PartialEq)]
pub enum Real {
    /// IEEE binary64 (includes non-finite values).
    Machine(f64),
    /// Finite binary float with explicit working precision.
    BigFloat(Decimal),
}

impl Real {
    /// Machine real.
    pub fn machine(x: f64) -> Self {
        Self::Machine(x)
    }

    /// Arbitrary-precision finite real.
    pub fn big_float(b: Decimal) -> Self {
        Self::BigFloat(b)
    }

    /// Machine `f64` view (exact for [`Self::Machine`], exact when [`Decimal::to_f64_exact`] succeeds).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Machine(x) => Some(*x),
            Self::BigFloat(b) => b.to_f64_exact(),
        }
    }

    /// Whether the value is finite.
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Machine(x) => x.is_finite(),
            Self::BigFloat(_) => true,
        }
    }

    /// View as [`Decimal`] when already arbitrary.
    pub fn as_big_float(&self) -> Option<&Decimal> {
        match self {
            Self::BigFloat(b) => Some(b),
            _ => None,
        }
    }
}

impl Default for Real {
    fn default() -> Self {
        Self::Machine(0.0)
    }
}
