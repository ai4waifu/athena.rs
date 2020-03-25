//! 实数：IEEE binary64 或二进制 [`Decimal`]。

use crate::{execution_budget::NumericContext, representation::decimal::Decimal};

/// 实数值表示。
///
/// 不实现 [`Clone`]。深复制用 [`Self::try_clone_in`]。
#[derive(Debug, PartialEq)]
pub enum Real {
    /// IEEE binary64（含非有限值）。
    Machine(f64),
    /// 带显式工作精度的有限二进制浮点。
    Decimal(Decimal),
}

impl Real {
    /// Limb / 机器实数可栈拷贝时返回副本。
    pub fn clone_inline(&self) -> Option<Self> {
        match self {
            Self::Machine(x) => Some(Self::Machine(*x)),
            Self::Decimal(d) => Some(Self::Decimal(d.clone_inline()?)),
        }
    }

    /// Owning 深复制。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> athena_types::Result<Self> {
        Ok(match self {
            Self::Machine(x) => Self::Machine(*x),
            Self::Decimal(d) => Self::Decimal(d.try_clone_in(ctx)?),
        })
    }

    /// 机器实数。
    pub fn machine(x: f64) -> Self {
        Self::Machine(x)
    }

    /// 任意精度有限实数。
    pub fn decimal(b: Decimal) -> Self {
        Self::Decimal(b)
    }

    /// 机器 `f64` 视图（[`Self::Machine`] 精确；[`Decimal::to_f64_exact`] 成功时精确）。
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Machine(x) => Some(*x),
            Self::Decimal(b) => b.to_f64_exact(),
        }
    }

    /// 值是否有限。
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Machine(x) => x.is_finite(),
            Self::Decimal(_) => true,
        }
    }

    /// 是否为 IEEE NaN（仅 [`Self::Machine`]）。
    pub fn is_nan(&self) -> bool {
        matches!(self, Self::Machine(x) if x.is_nan())
    }

    /// 已是任意精度时视为 [`Decimal`]。
    pub fn as_decimal(&self) -> Option<&Decimal> {
        match self {
            Self::Decimal(b) => Some(b),
            _ => None,
        }
    }
}

impl Default for Real {
    fn default() -> Self {
        Self::Machine(0.0)
    }
}
