//! 区间算术骨架：强制不变量与定向舍入。

use std::cmp::Ordering;

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    precision::PrecisionKind,
    real::Real,
    rounding::{f64_add_down, f64_add_up, f64_div_down, f64_div_up, f64_mul_down, f64_mul_up, f64_sub_down, f64_sub_up},
};

/// IEEE 1788 风格装饰（骨架）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntervalDecoration {
    /// 确定。
    Certain,
    /// 已定义。
    Defined,
    /// 平凡。
    #[default]
    Trivial,
    /// 病态。
    Ill,
}

/// 带强制不变量的实区间。
///
/// 不变量：
/// - [`Interval::Empty`] 为规范空集。
/// - [`Interval::Entire`] 为规范无界全集。
/// - [`Interval::Bounded`] 要求有限或无穷端点且 `lower ≤ upper`，无 NaN。
#[derive(Debug, Clone, PartialEq)]
pub enum Interval {
    /// 空集（规范；勿用倒置边界编码）。
    Empty,
    /// 整个实线 `(-∞, +∞)`。
    Entire {
        /// 装饰。
        decoration: IntervalDecoration,
    },
    /// 闭有界区间 `[lower, upper]`（单侧无界时端点可为 ±∞）。
    Bounded {
        /// 下端点（计算时定向舍入朝 −∞）。
        lower: Real,
        /// 上端点（计算时定向舍入朝 +∞）。
        upper: Real,
        /// 装饰。
        decoration: IntervalDecoration,
    },
}

impl Interval {
    /// 空区间。
    pub fn empty() -> Self {
        Self::Empty
    }

    /// 整个实线。
    pub fn entire() -> Self {
        Self::Entire { decoration: IntervalDecoration::Trivial }
    }

    /// 带装饰的全集。
    pub fn entire_with(decoration: IntervalDecoration) -> Self {
        Self::Entire { decoration }
    }

    /// 点区间 `[x, x]`。
    pub fn try_point(x: Real) -> Result<Self> {
        if !x.is_finite() {
            return Err(invalid("interval_point_non_finite"));
        }
        Self::try_bounded(x.clone(), x, IntervalDecoration::Certain)
    }

    /// 带校验的有界区间。
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

    /// 用外向舍入包络单个机器值。
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

    /// 装饰。
    pub fn decoration(&self) -> IntervalDecoration {
        match self {
            Self::Empty => IntervalDecoration::Trivial,
            Self::Entire { decoration } | Self::Bounded { decoration, .. } => *decoration,
        }
    }

    /// 区间是否为空。
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// 区间是否为整个实线。
    pub fn is_entire(&self) -> bool {
        matches!(self, Self::Entire { .. })
    }

    /// 有界或单侧无界时的下端点。
    pub fn lower(&self) -> Option<&Real> {
        match self {
            Self::Bounded { lower, .. } => Some(lower),
            Self::Empty | Self::Entire { .. } => None,
        }
    }

    /// 有界或单侧无界时的上端点。
    pub fn upper(&self) -> Option<&Real> {
        match self {
            Self::Bounded { upper, .. } => Some(upper),
            Self::Empty | Self::Entire { .. } => None,
        }
    }

    /// 两端点均可表示时的机器 `f64` 边界。
    pub fn as_f64_bounds(&self) -> Option<(f64, f64)> {
        match self {
            Self::Bounded { lower, upper, .. } => Some((lower.as_f64()?, upper.as_f64()?)),
            _ => None,
        }
    }

    /// 精度种类提示。
    pub fn precision_kind(&self) -> PrecisionKind {
        PrecisionKind::Interval
    }

    /// 机器端点上带定向舍入的区间加法。
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

    /// 带定向舍入的区间减法。
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

    /// 带定向舍入的区间乘法（仅机器端点）。
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

    /// 带定向舍入的区间除法（仅机器端点）。
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

    /// 取负：`[-upper, -lower]`（定向舍入）。
    pub fn neg(&self) -> Result<Self> {
        match self {
            Self::Empty => Ok(Self::Empty),
            Self::Entire { decoration } => Ok(Self::Entire { decoration: *decoration }),
            Self::Bounded { lower, upper, decoration } => {
                let lo = Real::machine(f64_sub_down(0.0, endpoint_f64(upper, "interval_neg")?));
                let hi = Real::machine(f64_sub_up(0.0, endpoint_f64(lower, "interval_neg")?));
                Self::try_bounded(lo, hi, *decoration)
            }
        }
    }

    /// 是否为点区间 `[x, x]`。
    pub fn is_point(&self) -> bool {
        match self {
            Self::Bounded { lower, upper, .. } => lower == upper,
            Self::Empty | Self::Entire { .. } => false,
        }
    }

    /// `x` 是否落在区间内（仅机器端点）。
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
