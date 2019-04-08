//! [`Natural`] 查询与位检验。

use super::Natural;
use crate::storage::Mode;

impl Natural {
    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        matches!(self.inner.mode(), Mode::Limb1) && self.as_limbs() == [1]
    }

    /// 最低 limb 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && (self.as_limbs()[0] & 1) == 1
    }

    /// 二进制位宽（零 → 0）。
    pub fn bits(&self) -> u64 {
        if self.is_zero() {
            return 0;
        }
        let limbs = self.as_limbs();
        let top = limbs.len() - 1;
        (top as u64) * 64 + (64 - limbs[top].leading_zeros() as u64)
    }

    /// 测试第 `index` 位（0 = LSB）。越界为 false。
    pub(crate) fn bit(&self, index: u64) -> bool {
        let limb_i = (index / 64) as usize;
        let limbs = self.as_limbs();
        if limb_i >= limbs.len() {
            return false;
        }
        let bit_i = (index % 64) as u32;
        (limbs[limb_i] >> bit_i) & 1 == 1
    }

    /// 是否有低于 `bit_index` 的任意置位（即 bits `[0, bit_index)`）。
    pub(crate) fn any_bits_below(&self, bit_index: u64) -> bool {
        if bit_index == 0 || self.is_zero() {
            return false;
        }
        let full_limbs = (bit_index / 64) as usize;
        let rem_bits = (bit_index % 64) as u32;
        let limbs = self.as_limbs();
        let el = limbs.len();
        let scan = full_limbs.min(el);
        for &limb in &limbs[..scan] {
            if limb != 0 {
                return true;
            }
        }
        if rem_bits > 0 && full_limbs < el {
            let mask = (1u64 << rem_bits) - 1;
            if limbs[full_limbs] & mask != 0 {
                return true;
            }
        }
        false
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        if self.is_zero() {
            return false;
        }
        let mut ones = 0u32;
        for &limb in self.as_limbs() {
            ones += limb.count_ones();
            if ones > 1 {
                return false;
            }
        }
        ones == 1
    }

    /// 可落入 `u64` 时返回（零 → `Some(0)`）。
    pub fn to_u64(&self) -> Option<u64> {
        if self.is_zero() {
            return Some(0);
        }
        let limbs = self.as_limbs();
        if limbs.len() == 1 { Some(limbs[0]) } else { None }
    }

    /// 可落入 `u128` 时返回。
    pub fn to_u128(&self) -> Option<u128> {
        if self.is_zero() {
            return Some(0);
        }
        let limbs = self.as_limbs();
        match limbs.len() {
            1 => Some(limbs[0] as u128),
            2 => Some(limbs[0] as u128 | ((limbs[1] as u128) << 64)),
            _ => None,
        }
    }
}
