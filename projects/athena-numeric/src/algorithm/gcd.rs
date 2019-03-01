//! GCD 策略选择（Euclid/Lehmer vs binary；half-GCD 能力位预留）。

use crate::dispatch::AlgorithmCapability;

/// 启用 Lehmer 加速的最小操作数宽度（limb）；低于此走 binary GCD。
pub const GCD_LEHMER_THRESHOLD: usize = 3;

/// GCD 算法族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GcdStrategy {
    /// Stein binary GCD。
    Binary,
    /// Lehmer 加速 Euclid，收尾 binary。
    Lehmer,
    /// Half-GCD（能力位未开时不可选）。
    HalfGcd,
}

/// 按操作数宽度与算法能力选择 GCD 策略。
pub(crate) fn select_gcd_strategy(a_limbs: usize, b_limbs: usize, caps: AlgorithmCapability) -> GcdStrategy {
    let n = a_limbs.min(b_limbs);
    if n == 0 {
        return GcdStrategy::Binary;
    }
    if caps.half_gcd && n >= GCD_LEHMER_THRESHOLD * 4 {
        return GcdStrategy::HalfGcd;
    }
    if a_limbs >= GCD_LEHMER_THRESHOLD && b_limbs >= GCD_LEHMER_THRESHOLD {
        GcdStrategy::Lehmer
    }
    else {
        GcdStrategy::Binary
    }
}
