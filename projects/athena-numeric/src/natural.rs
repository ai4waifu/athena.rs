//! 非负大整数（纯 Rust  limb 表示；`num-*` 迁移层）。

const BASE: u128 = 1u128 << 64;

/// 自然数（小端 `u64` limb，无尾随零）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Natural {
    limbs: Vec<u64>,
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
        self.limbs.len() == 1 && self.limbs[0] == 0
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        self.limbs.len() == 1 && self.limbs[0] == 1
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
        let top = self.limbs.len() - 1;
        (top as u64) * 64 + (64 - self.limbs[top].leading_zeros() as u64)
    }

    /// 右移一位（整除 2）。
    pub fn div2(&mut self) {
        if self.is_zero() {
            return;
        }
        let mut carry = 0u64;
        for i in (0..self.limbs.len()).rev() {
            let limb = self.limbs[i];
            let new_carry = limb & 1;
            self.limbs[i] = (limb >> 1) | (carry << 63);
            carry = new_carry;
        }
        while self.limbs.len() > 1 && self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.limbs.push(0);
        }
    }

    /// 加小整数。
    pub fn add_u64(&self, mut rhs: u64) -> Self {
        if rhs == 0 {
            return self.clone();
        }
        let mut out = self.limbs.clone();
        let mut i = 0usize;
        while rhs > 0 || i < out.len() {
            if i >= out.len() {
                out.push(0);
            }
            let sum = out[i] as u128 + rhs as u128;
            out[i] = sum as u64;
            rhs = (sum >> 64) as u64;
            i += 1;
        }
        Self { limbs: out }.normalize()
    }

    /// 乘小整数。
    pub fn mul_u64(&self, rhs: u64) -> Self {
        if self.is_zero() || rhs == 0 {
            return Self::zero();
        }
        if rhs == 1 {
            return self.clone();
        }
        let mut out = vec![0u64; self.limbs.len() + 1];
        let mut carry = 0u128;
        for (i, &limb) in self.limbs.iter().enumerate() {
            let prod = limb as u128 * rhs as u128 + carry;
            out[i] = prod as u64;
            carry = prod >> 64;
        }
        if carry > 0 {
            out[self.limbs.len()] = carry as u64;
        }
        else {
            out.pop();
        }
        Self { limbs: out }.normalize()
    }

    /// 加法。
    pub fn add(&self, rhs: &Self) -> Self {
        let max = self.limbs.len().max(rhs.limbs.len());
        let mut out = Vec::with_capacity(max + 1);
        let mut carry = 0u128;
        for i in 0..max {
            let a = *self.limbs.get(i).unwrap_or(&0) as u128;
            let b = *rhs.limbs.get(i).unwrap_or(&0) as u128;
            let sum = a + b + carry;
            out.push(sum as u64);
            carry = sum >> 64;
        }
        if carry > 0 {
            out.push(carry as u64);
        }
        Self { limbs: out }.normalize()
    }

    /// 减法（要求 `self >= rhs`）。
    pub fn sub(&self, rhs: &Self) -> Self {
        assert!(self >= rhs);
        let mut out = self.limbs.clone();
        let mut borrow = 0i128;
        for i in 0..out.len() {
            let a = out[i] as i128 - borrow;
            let b = *rhs.limbs.get(i).unwrap_or(&0) as i128;
            if a >= b {
                out[i] = (a - b) as u64;
                borrow = 0;
            }
            else {
                out[i] = (a + BASE as i128 - b) as u64;
                borrow = 1;
            }
        }
        Self { limbs: out }.normalize()
    }

    /// 乘法（schoolbook）。
    pub fn mul(&self, rhs: &Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::zero();
        }
        let mut out = vec![0u64; self.limbs.len() + rhs.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &b) in rhs.limbs.iter().enumerate() {
                let idx = i + j;
                let prod = a as u128 * b as u128 + out[idx] as u128 + carry;
                out[idx] = prod as u64;
                carry = prod >> 64;
            }
            if carry > 0 {
                out[i + rhs.limbs.len()] = out[i + rhs.limbs.len()].wrapping_add(carry as u64);
            }
        }
        Self { limbs: out }.normalize()
    }

    /// 除法与余数（`rhs > 0`）。
    pub fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        assert!(!rhs.is_zero());
        if self < rhs {
            return (Self::zero(), self.clone());
        }
        if rhs.is_one() {
            return (self.clone(), Self::zero());
        }
        let mut quotient = Self::zero();
        let mut remainder = Self::zero();
        for i in (0..self.bits()).rev() {
            remainder = remainder.mul_u64(2);
            let bit_idx = i as usize;
            let limb = bit_idx / 64;
            let bit = bit_idx % 64;
            if limb < self.limbs.len() && ((self.limbs[limb] >> bit) & 1) == 1 {
                remainder = remainder.add_u64(1);
            }
            if remainder >= *rhs {
                remainder = remainder.sub(rhs);
                quotient.set_bit(bit_idx);
            }
        }
        (quotient.normalize(), remainder.normalize())
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
        for &limb in &self.limbs {
            ones += limb.count_ones();
            if ones > 1 {
                return false;
            }
        }
        ones == 1
    }

    /// 十进制解析（仅数字，无符号）。
    pub fn from_decimal_digits(digits: &str) -> Result<Self, ()> {
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

    /// 可落入 `u64` 时返回。
    pub fn to_u64(&self) -> Option<u64> {
        if self.limbs.len() == 1 { Some(self.limbs[0]) } else { None }
    }

    /// 可落入 `u128` 时返回。
    pub fn to_u128(&self) -> Option<u128> {
        match self.limbs.len() {
            0 | 1 if self.is_zero() => Some(0),
            1 => Some(self.limbs[0] as u128),
            2 => Some(self.limbs[0] as u128 | ((self.limbs[1] as u128) << 64)),
            _ => None,
        }
    }

    fn set_bit(&mut self, idx: usize) {
        let limb = idx / 64;
        let bit = idx % 64;
        if limb >= self.limbs.len() {
            self.limbs.resize(limb + 1, 0);
        }
        self.limbs[limb] |= 1u64 << bit;
    }

    fn normalize(mut self) -> Self {
        while self.limbs.len() > 1 && self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.limbs.push(0);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_arith() {
        let a = Natural::from_decimal_digits("12345").unwrap();
        let b = Natural::from_decimal_digits("67890").unwrap();
        assert_eq!(a.add(&b).to_decimal_string(), "80235");
        assert_eq!(b.sub(&a).to_decimal_string(), "55545");
        assert_eq!(a.mul_u64(10).to_decimal_string(), "123450");
        let (q, r) = Natural::from_decimal_digits("17").unwrap().div_rem(&Natural::from_u64(5));
        assert_eq!(q.to_decimal_string(), "3");
        assert_eq!(r.to_decimal_string(), "2");
    }

    #[test]
    fn mod_pow_smoke() {
        let base = Natural::from_u64(3);
        let exp = Natural::from_u64(4);
        let m = Natural::from_u64(7);
        assert_eq!(base.mod_pow(&exp, &m).to_decimal_string(), "4");
    }
}
