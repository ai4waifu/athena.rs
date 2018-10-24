//! Interval arithmetic skeleton with enforced invariants and directed rounding.

use std::cmp::Ordering;

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    precision::PrecisionKind,
    real::Real,
    rounding::{f64_add_down, f64_add_up, f64_div_down, f64_div_up, f64_mul_down, f64_mul_up, f64_sub_down, f64_sub_up},
};

/// IEEE 1788-style decoration (skeleton).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntervalDecoration {
    /// Certain.
    Certain,
    /// Defined.
    Defined,
    /// Trivial.
    #[default]
    Trivial,
    /// Ill-conditioned.
    Ill,
}

/// Real interval with enforced invariants.
///
/// Invariants:
/// - [`Interval::Empty`] is the canonical empty set.
/// - [`Interval::Entire`] is the canonical unbounded set.
/// - [`Interval::Bounded`] requires finite-or-infinite endpoints with `lower <= upper`, no NaN.
#[derive(Debug, Clone, PartialEq)]
pub enum Interval {
    /// Empty set (canonical; do not encode as inverted bounds).
    Empty,
    /// Entire real line `(-∞, +∞)`.
    Entire {
        /// Decoration.
        decoration: IntervalDecoration,
    },
    /// Closed bounded interval `[lower, upper]` (endpoints may be ±∞ for one-sided unbounded sets).
    Bounded {
        /// Lower endpoint (directed rounding: toward −∞ when computed).
        lower: Real,
        /// Upper endpoint (directed rounding: toward +∞ when computed).
        upper: Real,
        /// Decoration.
        decoration: IntervalDecoration,
    },
}

impl Interval {
    /// Empty interval.
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Entire real line.
    pub fn entire() -> Self {
        Self::Entire { decoration: IntervalDecoration::Trivial }
    }

    /// Entire with decoration.
    pub fn entire_with(decoration: IntervalDecoration) -> Self {
        Self::Entire { decoration }
    }

    /// Point interval `[x, x]`.
    pub fn try_point(x: Real) -> Result<Self> {
        if !x.is_finite() {
            return Err(invalid("interval_point_non_finite"));
        }
        Self::try_bounded(x.clone(), x, IntervalDecoration::Certain)
    }

    /// Bounded interval with validation.
    pub fn try_bounded(lower: Real, upper: Real, decoration: IntervalDecoration) -> Result<Self> {
        let lo = endpoint_f64(&lower, "interval_lower")?;
        let hi = endpoint_f64(&upper, "interval_upper")?;
        if lo.is_nan() || hi.is_nan() {
            return Err(invalid("interval_nan_endpoint"));
        }
        match lo.partial_cmp(&hi) {
            Some(Ordering::Greater) => Err(invalid("interval_inverted_bounds")),
            Some(_) => {
                if lo.is_infinite() && lo.is_sign_negative() && hi.is_infinite() && hi.is_sign_positive() {
                    Ok(Self::Entire { decoration })
                }
                else {
                    Ok(Self::Bounded { lower, upper, decoration })
                }
            }
            None => Err(invalid("interval_uncomparable_bounds")),
        }
    }

    /// Enclose a single machine value with outward rounding.
    pub fn try_enclose_f64(x: f64) -> Result<Self> {
        if x.is_nan() {
            return Err(invalid("interval_enclose_nan"));
        }
        if x.is_infinite() {
            return if x.is_sign_negative() {
                Self::try_bounded(Real::machine(f64::NEG_INFINITY), Real::machine(f64::MAX), IntervalDecoration::Defined)
            }
            else {
                Self::try_bounded(Real::machine(f64::MIN), Real::machine(f64::INFINITY), IntervalDecoration::Defined)
            };
        }
        Self::try_point(Real::machine(x))
    }

    /// Decoration.
    pub fn decoration(&self) -> IntervalDecoration {
        match self {
            Self::Empty => IntervalDecoration::Trivial,
            Self::Entire { decoration } | Self::Bounded { decoration, .. } => *decoration,
        }
    }

    /// Whether the interval is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Whether the interval is the entire real line.
    pub fn is_entire(&self) -> bool {
        matches!(self, Self::Entire { .. })
    }

    /// Lower endpoint if bounded or one-sided unbounded.
    pub fn lower(&self) -> Option<&Real> {
        match self {
            Self::Bounded { lower, .. } => Some(lower),
            Self::Empty | Self::Entire { .. } => None,
        }
    }

    /// Upper endpoint if bounded or one-sided unbounded.
    pub fn upper(&self) -> Option<&Real> {
        match self {
            Self::Bounded { upper, .. } => Some(upper),
            Self::Empty | Self::Entire { .. } => None,
        }
    }

    /// Machine `f64` bounds when both endpoints are representable.
    pub fn as_f64_bounds(&self) -> Option<(f64, f64)> {
        match self {
            Self::Bounded { lower, upper, .. } => Some((lower.as_f64()?, upper.as_f64()?)),
            _ => None,
        }
    }

    /// Precision kind hint.
    pub fn precision_kind(&self) -> PrecisionKind {
        PrecisionKind::Interval
    }

    /// Interval addition with directed rounding on machine endpoints.
    pub fn add(&self, other: &Self) -> Result<Self> {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Ok(Self::Empty),
            (Self::Entire { decoration }, _) | (_, Self::Entire { decoration }) => Ok(Self::Entire { decoration: *decoration }),
            (
                Self::Bounded { lower: l1, upper: u1, decoration: d1 },
                Self::Bounded { lower: l2, upper: u2, decoration: d2 },
            ) => {
                let lo = Real::machine(f64_add_down(
                    endpoint_f64(l1, "interval_add_lower")?,
                    endpoint_f64(l2, "interval_add_lower")?,
                ));
                let hi =
                    Real::machine(f64_add_up(endpoint_f64(u1, "interval_add_upper")?, endpoint_f64(u2, "interval_add_upper")?));
                Self::try_bounded(lo, hi, merge_decoration(*d1, *d2))
            }
        }
    }

    /// Interval subtraction with directed rounding.
    pub fn sub(&self, other: &Self) -> Result<Self> {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Ok(Self::Empty),
            (Self::Entire { decoration }, Self::Entire { .. }) => Ok(Self::Entire { decoration: *decoration }),
            (Self::Entire { .. }, Self::Bounded { .. }) => Ok(Self::Entire { decoration: IntervalDecoration::Trivial }),
            (Self::Bounded { .. }, Self::Entire { .. }) => Ok(Self::Entire { decoration: IntervalDecoration::Trivial }),
            (
                Self::Bounded { lower: l1, upper: u1, decoration: d1 },
                Self::Bounded { lower: l2, upper: u2, decoration: d2 },
            ) => {
                let lo = Real::machine(f64_sub_down(
                    endpoint_f64(l1, "interval_sub_lower")?,
                    endpoint_f64(u2, "interval_sub_lower")?,
                ));
                let hi =
                    Real::machine(f64_sub_up(endpoint_f64(u1, "interval_sub_upper")?, endpoint_f64(l2, "interval_sub_upper")?));
                Self::try_bounded(lo, hi, merge_decoration(*d1, *d2))
            }
        }
    }

    /// Interval multiplication with directed rounding (machine endpoints only).
    pub fn mul(&self, other: &Self) -> Result<Self> {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Ok(Self::Empty),
            (Self::Entire { .. }, _) | (_, Self::Entire { .. }) => Ok(Self::Entire { decoration: IntervalDecoration::Trivial }),
            (
                Self::Bounded { lower: l1, upper: u1, decoration: d1 },
                Self::Bounded { lower: l2, upper: u2, decoration: d2 },
            ) => {
                let a = endpoint_f64(l1, "interval_mul")?;
                let b = endpoint_f64(u1, "interval_mul")?;
                let c = endpoint_f64(l2, "interval_mul")?;
                let d = endpoint_f64(u2, "interval_mul")?;
                let products = [f64_mul_down(a, c), f64_mul_down(a, d), f64_mul_down(b, c), f64_mul_down(b, d)];
                let products_up = [f64_mul_up(a, c), f64_mul_up(a, d), f64_mul_up(b, c), f64_mul_up(b, d)];
                let lo = products.into_iter().fold(f64::INFINITY, f64::min);
                let hi = products_up.into_iter().fold(f64::NEG_INFINITY, f64::max);
                Self::try_bounded(Real::machine(lo), Real::machine(hi), merge_decoration(*d1, *d2))
            }
        }
    }

    /// Interval division with directed rounding (machine endpoints only).
    pub fn div(&self, other: &Self) -> Result<Self> {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Ok(Self::Empty),
            (_, Self::Entire { .. }) => Ok(Self::Entire { decoration: IntervalDecoration::Trivial }),
            (Self::Entire { .. }, Self::Bounded { lower, upper, .. }) => {
                if contains_zero(lower, upper)? {
                    return Ok(Self::Entire { decoration: IntervalDecoration::Ill });
                }
                Ok(Self::Entire { decoration: IntervalDecoration::Trivial })
            }
            (
                Self::Bounded { lower: l1, upper: u1, decoration: d1 },
                Self::Bounded { lower: l2, upper: u2, decoration: d2 },
            ) => {
                if contains_zero(l2, u2)? {
                    return Err(invalid("interval_div_zero"));
                }
                let a = endpoint_f64(l1, "interval_div")?;
                let b = endpoint_f64(u1, "interval_div")?;
                let c = endpoint_f64(l2, "interval_div")?;
                let d = endpoint_f64(u2, "interval_div")?;
                let quotients = [f64_div_down(a, c), f64_div_down(a, d), f64_div_down(b, c), f64_div_down(b, d)];
                let quotients_up = [f64_div_up(a, c), f64_div_up(a, d), f64_div_up(b, c), f64_div_up(b, d)];
                let lo = quotients.into_iter().fold(f64::INFINITY, f64::min);
                let hi = quotients_up.into_iter().fold(f64::NEG_INFINITY, f64::max);
                Self::try_bounded(Real::machine(lo), Real::machine(hi), merge_decoration(*d1, *d2))
            }
        }
    }

    /// Whether `x` is contained in the interval (machine endpoints only).
    pub fn contains_f64(&self, x: f64) -> Result<bool> {
        if x.is_nan() {
            return Ok(false);
        }
        match self {
            Self::Empty => Ok(false),
            Self::Entire { .. } => Ok(true),
            Self::Bounded { lower, upper, .. } => {
                let lo = endpoint_f64(lower, "interval_contains")?;
                let hi = endpoint_f64(upper, "interval_contains")?;
                Ok(lo <= x && x <= hi)
            }
        }
    }
}

fn endpoint_f64(r: &Real, op: &str) -> Result<f64> {
    r.as_f64().ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "numeric").detail("operation", op)
    })
}

fn contains_zero(lower: &Real, upper: &Real) -> Result<bool> {
    let lo = endpoint_f64(lower, "interval_contains_zero")?;
    let hi = endpoint_f64(upper, "interval_contains_zero")?;
    Ok(lo <= 0.0 && 0.0 <= hi)
}

fn merge_decoration(a: IntervalDecoration, b: IntervalDecoration) -> IntervalDecoration {
    if a == IntervalDecoration::Ill || b == IntervalDecoration::Ill {
        IntervalDecoration::Ill
    }
    else if a == IntervalDecoration::Certain && b == IntervalDecoration::Certain {
        IntervalDecoration::Certain
    }
    else {
        IntervalDecoration::Defined
    }
}

fn invalid(operation: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", operation)
}
