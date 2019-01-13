//! 实数：IEEE binary64 或二进制 [`Decimal`]。

use crate::decimal::Decimal;

/// 实数值表示。
#[derive(Debug, Clone, PartialEq)]
pub enum Real {
    /// IEEE binary64（含非有限值）。
    Machine(f64),
    /// 带显式工作精度的有限二进制浮点。
    BigFloat(Decimal),
}

impl Real {
    /// 机器实数。
    pub fn machine(x: f64) -> Self {
        Self::Machine(x)
    }

    /// 任意精度有限实数。
    pub fn big_float(b: Decimal) -> Self {
        Self::BigFloat(b)
    }

    /// 机器 `f64` 视图（[`Self::Machine`] 精确；[`Decimal::to_f64_exact`] 成功时精确）。
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Machine(x) => Some(*x),
            Self::BigFloat(b) => b.to_f64_exact(),
        }
    }

    /// 值是否有限。
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Machine(x) => x.is_finite(),
            Self::BigFloat(_) => true,
        }
    }

    /// 已是任意精度时视为 [`Decimal`]。
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
