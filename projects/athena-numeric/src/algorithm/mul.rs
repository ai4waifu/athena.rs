//! 乘法策略选择（与 machine kernel 正交）。

use crate::dispatch::AlgorithmCapability;

/// Karatsuba 阈值（limb 数）；低于此走 schoolbook。
pub const MUL_KARATSUBA_THRESHOLD: usize = 32;

/// 乘法算法族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MulStrategy {
    /// 直接写零（任一操作数为零）。
    Zero,
    /// Schoolbook。
    Schoolbook,
    /// Karatsuba。
    Karatsuba,
}

/// 按操作数宽度与算法能力选择乘法策略。
pub fn select_mul_strategy(a_limbs: usize, b_limbs: usize, caps: AlgorithmCapability) -> MulStrategy {
    let n = a_limbs.max(b_limbs);
    if a_limbs == 0 || b_limbs == 0 {
        return MulStrategy::Zero;
    }
    if n >= MUL_KARATSUBA_THRESHOLD && caps.karatsuba {
        MulStrategy::Karatsuba
    }
    else if caps.schoolbook {
        MulStrategy::Schoolbook
    }
    else {
        // 合同要求至少 schoolbook；能力被关时仍回退 schoolbook 以免静默失败。
        MulStrategy::Schoolbook
    }
}

/// Karatsuba 递归所需 scratch limb 总数（逐层顺序复用 `rest`）。
///
/// 切分用 `m = ceil(n/2)`，**不用** `next_power_of_two`。
pub fn karatsuba_scratch_limbs(n_limbs: usize) -> usize {
    if n_limbs < MUL_KARATSUBA_THRESHOLD {
        return 0;
    }
    let m = (n_limbs + 1) / 2;
    let level = 2 * m + 2 * m + (m + 1) + (m + 1) + (2 * m + 2);
    level.saturating_add(karatsuba_scratch_limbs(m + 1))
}
