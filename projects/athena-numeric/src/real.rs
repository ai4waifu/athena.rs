//! 实数（机器 / 任意精度骨架）。

use crate::precision::PrecisionInfo;

/// 实数表示。
#[derive(Debug, Clone, PartialEq)]
pub enum Real {
    /// IEEE binary64。
    Machine(f64),
    /// 任意精度占位（payload 后续接 backend）。
    Arbitrary {
        /// 十进制或内部编码占位。
        decimal: String,
        /// 精度。
        precision: PrecisionInfo,
    },
}

impl Real {
    /// 机器实数。
    pub fn machine(x: f64) -> Self {
        Self::Machine(x)
    }

    /// 由十进制字符串构造任意精度占位。
    pub fn arbitrary_decimal(decimal: impl Into<String>, bits: u32) -> Self {
        Self::Arbitrary { decimal: decimal.into(), precision: PrecisionInfo::arbitrary(bits) }
    }

    /// 精度信息。
    pub fn precision(&self) -> PrecisionInfo {
        match self {
            Self::Machine(_) => PrecisionInfo::machine(),
            Self::Arbitrary { precision, .. } => precision.clone(),
        }
    }

    /// 是否有限。
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Machine(x) => x.is_finite(),
            Self::Arbitrary { .. } => true,
        }
    }
}

impl Default for Real {
    fn default() -> Self {
        Self::Machine(0.0)
    }
}
