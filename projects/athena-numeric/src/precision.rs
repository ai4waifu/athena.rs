//! 精度信息。

/// 精度种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PrecisionKind {
    /// 精确。
    #[default]
    Exact,
    /// IEEE 机器精度。
    Machine,
    /// 任意精度。
    Arbitrary,
    /// 区间包络。
    Interval,
    /// 带证书。
    Certified,
}

/// 一等精度对象。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrecisionInfo {
    /// 种类。
    pub kind: PrecisionKind,
    /// 比特精度（若适用）。
    pub bits: Option<u32>,
    /// 十进制位数（若适用）。
    pub decimal_digits: Option<u32>,
    /// 是否保证（证书路径）。
    pub guaranteed: bool,
}

impl PrecisionInfo {
    /// 精确值。
    pub fn exact() -> Self {
        Self { kind: PrecisionKind::Exact, bits: None, decimal_digits: None, guaranteed: true }
    }

    /// 机器实数。
    pub fn machine() -> Self {
        Self { kind: PrecisionKind::Machine, bits: Some(53), decimal_digits: None, guaranteed: false }
    }

    /// 任意精度（比特）。
    pub fn arbitrary(bits: u32) -> Self {
        Self { kind: PrecisionKind::Arbitrary, bits: Some(bits), decimal_digits: None, guaranteed: false }
    }
}
