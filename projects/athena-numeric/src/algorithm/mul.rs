//! 乘法策略选择（与 machine kernel 正交）。

use crate::dispatch::AlgorithmCapability;

/// Karatsuba 阈值（limb 数）；低于此走 schoolbook。
pub const MUL_KARATSUBA_THRESHOLD: usize = 32;

/// Toom-3 阈值（limb 数）；低于此走 Karatsuba（若启用）。
pub const MUL_TOOM_THRESHOLD: usize = 96;

/// 乘法算法族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MulStrategy {
    /// 直接写零（任一操作数为零）。
    Zero,
    /// Schoolbook。
    Schoolbook,
    /// Karatsuba。
    Karatsuba,
    /// Toom-3。
    Toom3,
}

/// 按操作数宽度与算法能力选择乘法策略。
pub(crate) fn select_mul_strategy(a_limbs: usize, b_limbs: usize, caps: AlgorithmCapability) -> MulStrategy {
    let n = a_limbs.max(b_limbs);
    if a_limbs == 0 || b_limbs == 0 {
        return MulStrategy::Zero;
    }
    if n >= MUL_TOOM_THRESHOLD && caps.toom {
        return MulStrategy::Toom3;
    }
    if n >= MUL_KARATSUBA_THRESHOLD && caps.karatsuba {
        return MulStrategy::Karatsuba;
    }
    if caps.schoolbook { MulStrategy::Schoolbook } else { MulStrategy::Schoolbook }
}

/// Karatsuba 递归所需 scratch limb 总数（逐层顺序复用 `rest`）。
pub fn karatsuba_scratch_limbs(n_limbs: usize) -> usize {
    if n_limbs < MUL_KARATSUBA_THRESHOLD {
        return 0;
    }
    if n_limbs >= MUL_TOOM_THRESHOLD {
        // Toom 路径另计；此处仍给 Karatsuba 上界以免低估。
    }
    let m = (n_limbs + 1) / 2;
    let level = 2 * m + 2 * m + (m + 1) + (m + 1) + (2 * m + 2);
    level.saturating_add(karatsuba_scratch_limbs(m + 1))
}

/// Toom-3 scratch 上界（五点求值/插值 + 子 `mul_rec`）。
pub fn toom3_scratch_limbs(n_limbs: usize) -> usize {
    if n_limbs < MUL_TOOM_THRESHOLD {
        return karatsuba_scratch_limbs(n_limbs);
    }
    let m = (n_limbs + 2) / 3;
    let eval = m + 2;
    let prod = 2 * eval;
    let level = 5 * prod + 11 * eval + 4 * prod;
    level.saturating_add(karatsuba_scratch_limbs(eval))
}
