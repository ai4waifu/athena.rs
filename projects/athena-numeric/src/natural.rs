//! 非负大整数（纯 Rust limb 表示；算法委托 [`crate::limbs`] 内核）。

use std::{cmp::Ordering, str::FromStr};

/// 自然数（小端 `u64` limb，无尾随零）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Natural {
    limbs: Vec<u64>,
}

impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        crate::limbs::cmp_slice(&self.limbs, &other.limbs)
    }
}

impl Natural {
    /// 零。
    pub fn zero() -> Self {
        Self { limbs: vec![0] }
    }

    /// 一。
    pub fn one() -> Self {
        Self { limbs: vec![1] }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        crate::limbs::is_zero(&self.limbs)
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        crate::limbs::effective_len(&self.limbs) == 1 && self.limbs[0] == 1
    }

    /// 最低 limb 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && (self.limbs[0] & 1) == 1
    }

    /// 由 `u64` 构造。
    pub fn from_u64(n: u64) -> Self {
        if n == 0 { Self::zero() } else { Self { limbs: vec![n] } }
    }

    /// 二进制位宽（零 → 0）。
    pub fn bits(&self) -> u64 {
        if self.is_zero() {
            return 0;
        }
        let top = crate::limbs::effective_len(&self.limbs) - 1;
        (top as u64) * 64 + (64 - self.limbs[top].leading_zeros() as u64)
    }

    /// 右移一位（整除 2）。
    pub fn div2(&mut self) {
        if self.is_zero() {
            return;
        }
        let mut carry = 0u64;
        let len = crate::limbs::effective_len(&self.limbs);
        for i in (0..len).rev() {
            let limb = self.limbs[i];
            let new_carry = limb & 1;
            self.limbs[i] = (limb >> 1) | (carry << 63);
            carry = new_carry;
        }
        self.limbs = crate::limbs::normalize_trim(std::mem::take(&mut self.limbs));
    }

    /// 加小整数。
    pub fn add_u64(&self, rhs: u64) -> Self {
        if rhs == 0 {
            return self.clone();
        }
        Self::from_limbs(crate::limbs::add_n(&self.limbs, &[rhs]))
    }

    /// 乘小整数。
    pub fn mul_u64(&self, rhs: u64) -> Self {
        if self.is_zero() || rhs == 0 {
            return Self::zero();
        }
        if rhs == 1 {
            return self.clone();
        }
        Self::from_limbs(crate::limbs::mul(&self.limbs, &[rhs]))
    }

    /// 加法。
    pub fn add(&self, rhs: &Self) -> Self {
        Self::from_limbs(crate::limbs::add_n(&self.limbs, &rhs.limbs))
    }

    /// 减法（要求 `self >= rhs`）。
    pub fn sub(&self, rhs: &Self) -> Self {
        assert!(self >= rhs);
        Self::from_limbs(crate::limbs::sub_n(&self.limbs, &rhs.limbs))
    }

    /// 乘法。
    pub fn mul(&self, rhs: &Self) -> Self {
        Self::from_limbs(crate::limbs::mul(&self.limbs, &rhs.limbs))
    }

    /// 除法与余数（`rhs > 0`）。
    pub fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        let (q, r) = crate::limbs::div_rem(&self.limbs, &rhs.limbs);
        (Self::from_limbs(q), Self::from_limbs(r))
    }

    /// 模幂（`modulus > 0`）。
    pub fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        assert!(!modulus.is_zero());
        if modulus.is_one() {
            return Self::zero();
        }
        let mut result = Self::one();
        let mut base = self.clone();
        base = base.div_rem(modulus).1;
        let mut e = exp.clone();
        while !e.is_zero() {
            if e.is_odd() {
                result = result.mul(&base).div_rem(modulus).1;
            }
            base = base.mul(&base).div_rem(modulus).1;
            e.div2();
        }
        result
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        if self.is_zero() {
            return false;
        }
        let mut ones = 0u32;
        let len = crate::limbs::effective_len(&self.limbs);
        for &limb in &self.limbs[..len] {
            ones += limb.count_ones();
            if ones > 1 {
                return false;
            }
        }
        ones == 1
    }

    /// 十进制字符串（无符号）。
    pub fn to_decimal_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let (mut q, mut r) = self.clone().div_rem(&Self::from_u64(10));
        let mut digits = vec![b'0' + r.limbs[0] as u8];
        while !q.is_zero() {
            (q, r) = q.div_rem(&Self::from_u64(10));
            digits.push(b'0' + r.limbs[0] as u8);
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_else(|_| "0".to_string())
    }

    /// 可落入 `u64` 时返回（零 → `Some(0)`）。
    pub fn to_u64(&self) -> Option<u64> {
        if self.is_zero() {
            return Some(0);
        }
        if crate::limbs::effective_len(&self.limbs) == 1 { Some(self.limbs[0]) } else { None }
    }

    /// 可落入 `u128` 时返回。
    pub fn to_u128(&self) -> Option<u128> {
        match crate::limbs::effective_len(&self.limbs) {
            0 | 1 if self.is_zero() => Some(0),
            1 => Some(self.limbs[0] as u128),
            2 => Some(self.limbs[0] as u128 | ((self.limbs[1] as u128) << 64)),
            _ => None,
        }
    }

    /// 非负最大公约数（binary GCD via limb kernel）。
    pub fn gcd(&self, other: &Self) -> Self {
        if self.is_zero() && other.is_zero() {
            return Self::zero();
        }
        Self::from_limbs(crate::limbs::binary_gcd(self.limbs.clone(), other.limbs.clone()))
    }

    /// Canonical limb buffer (crate-private; for limb kernel tests).
    pub(crate) fn from_limbs(limbs: Vec<u64>) -> Self {
        Self { limbs: crate::limbs::normalize_trim(limbs) }
    }
}

impl FromStr for Natural {
    type Err = ();

    /// 十进制解析（仅数字，无符号）。
    fn from_str(digits: &str) -> Result<Self, Self::Err> {
        if digits.is_empty() {
            return Err(());
        }
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let mut n = Self::zero();
        for ch in digits.chars() {
            n = n.mul_u64(10).add_u64(u64::from(ch as u32 - u32::from(b'0')));
        }
        Ok(n)
    }
}
