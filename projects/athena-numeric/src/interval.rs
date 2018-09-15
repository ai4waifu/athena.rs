//! 区间算术骨架。

use crate::precision::PrecisionKind;
use crate::real::Real;

/// IEEE 1788 风格 decoration（骨架）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IntervalDecoration {
    /// 确定。
    Certain,
    /// 有定义。
    Defined,
    /// 平凡。
    #[default]
    Trv,
    /// 病态。
    Ill,
}

/// 实区间。
#[derive(Debug, Clone, PartialEq)]
pub struct Interval {
    /// 下界。
    pub lower: Real,
    /// 上界。
    pub upper: Real,
    /// decoration。
    pub decoration: IntervalDecoration,
}

impl Interval {
    /// 点区间。
    pub fn point(x: Real) -> Self {
        Self {
            lower: x.clone(),
            upper: x,
            decoration: IntervalDecoration::Certain,
        }
    }

    /// 精度种类提示。
    pub fn precision_kind(&self) -> PrecisionKind {
        PrecisionKind::Interval
    }
}
