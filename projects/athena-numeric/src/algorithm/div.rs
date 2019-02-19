//! 除法策略选择（Knuth vs Burnikel–Ziegler）。

use crate::dispatch::AlgorithmCapability;

/// 启用 BZ 的被除数相对除数宽度阈值（limb）。
pub const DIV_BZ_THRESHOLD: usize = 64;

/// 除法算法族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DivStrategy {
    /// 经典 Knuth D（归一化长除）。
    Knuth,
    /// Burnikel–Ziegler（大宽度递归）。
    BurnikelZiegler,
}

/// 按操作数宽度与算法能力选择除法策略。
pub(crate) fn select_div_strategy(u_limbs: usize, v_limbs: usize, caps: AlgorithmCapability) -> DivStrategy {
    if v_limbs == 0 {
        return DivStrategy::Knuth;
    }
    if caps.bz_division && u_limbs >= DIV_BZ_THRESHOLD && u_limbs >= 2 * v_limbs {
        DivStrategy::BurnikelZiegler
    }
    else {
        DivStrategy::Knuth
    }
}
