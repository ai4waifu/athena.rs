//! 实数（机器 / 任意精度骨架）。

use crate::precision::{PrecisionInfo, PrecisionKind};

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

/// 确保 Arbitrary 变体使用 Arbitrary kind（骨架辅助）。
#[allow(dead_code)]
fn arbitrary_precision(bits: u32) -> PrecisionInfo {
    PrecisionInfo {
        kind: PrecisionKind::Arbitrary,
        bits: Some(bits),
        decimal_digits: None,
        guaranteed: false,
    }
}
