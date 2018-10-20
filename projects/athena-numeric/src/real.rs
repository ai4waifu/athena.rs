//! Real numbers (machine only until true BigFloat exists).
//!
//! Precision and rounding context are derived from the [`crate::number::NumericValue`] variant.
//! There is no public arbitrary-precision real yet: a tagged `f64` bit pattern must not
//! be advertised as `PrecisionKind::Arbitrary`.

/// Real value representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Real {
    /// IEEE binary64.
    Machine(f64),
    /// Capability / future-backend placeholder. Not a numeric execution payload.
    Unsupported,
}

impl Real {
    /// Machine real.
    pub fn machine(x: f64) -> Self {
        Self::Machine(x)
    }

    /// Machine `f64` view.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Machine(x) => Some(*x),
            Self::Unsupported => None,
        }
    }

    /// Whether the value is finite.
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Machine(x) => x.is_finite(),
            Self::Unsupported => false,
        }
    }
}

impl Default for Real {
    fn default() -> Self {
        Self::Machine(0.0)
    }
}
