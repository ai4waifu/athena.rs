//! 精确整数包装（纯 Rust 内部表示，不暴露 limb / `num-*`）。

use crate::natural::Natural;

/// 符号（精确）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sign {
    /// 负。
    Negative,
    /// 零。
    Zero,
    /// 正。
    Positive,
}

/// 精确整数（稳定公共包装；亦称 [`ExactInteger`]）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integer {
    sign: Sign,
    mag: Natural,
}

/// Living `16` 稳定别名。
pub type ExactInteger = Integer;

impl Integer {
    fn from_mag_sign(mag: Natural, negative: bool) -> Self {
        if mag.is_zero() {
            Self { sign: Sign::Zero, mag }
        }
        else if negative {
            Self { sign: Sign::Negative, mag }
        }
        else {
            Self { sign: Sign::Positive, mag }
        }
    }

    /// 由已解码 `i64` 构造。
    pub fn from_i64(n: i64) -> Self {
        if n == 0 {
            Self::zero()
        }
        else if n < 0 {
            Self::from_mag_sign(Natural::from_u64(n.unsigned_abs()), true)
        }
        else {
            Self::from_mag_sign(Natural::from_u64(n as u64), false)
        }
    }

    /// 由 `u64` 构造。
    pub fn from_u64(n: u64) -> Self {
        Self::from_mag_sign(Natural::from_u64(n), false)
    }

    /// 解析十进制字符串（可含前导 `+/-`）。
    pub fn from_decimal_str(s: &str) -> Result<Self, ()> {
        let t = s.trim();
        if t.is_empty() {
            return Err(());
        }
        let (negative, digits) = match t.as_bytes()[0] {
            b'+' => (false, &t[1..]),
            b'-' => (true, &t[1..]),
            _ => (false, t),
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let mag = Natural::from_decimal_digits(digits)?;
        Ok(Self::from_mag_sign(mag, negative))
    }

    /// 零。
    pub fn zero() -> Self {
        Self { sign: Sign::Zero, mag: Natural::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { sign: Sign::Positive, mag: Natural::one() }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.sign == Sign::Zero
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        self.sign == Sign::Positive && self.mag.is_one()
    }

    /// 是否为负。
    pub fn is_negative(&self) -> bool {
        self.sign == Sign::Negative
    }

    /// 是否为正。
    pub fn is_positive(&self) -> bool {
        self.sign == Sign::Positive
    }

    /// 是否非负（零视为非负）。
    pub fn is_non_negative(&self) -> bool {
        self.sign != Sign::Negative
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// 绝对值。
    pub fn abs(&self) -> Self {
        if self.is_zero() { Self::zero() } else { Self { sign: Sign::Positive, mag: self.mag.clone() } }
    }

    /// 取负。
    pub fn neg(&self) -> Self {
        match self.sign {
            Sign::Zero => Self::zero(),
            Sign::Positive => Self { sign: Sign::Negative, mag: self.mag.clone() },
            Sign::Negative => Self { sign: Sign::Positive, mag: self.mag.clone() },
        }
    }

    /// 非负最大公约数；`gcd(0,0) = 0`。
    pub fn gcd(&self, other: &Self) -> Self {
        let mut a = self.abs().mag;
        let mut b = other.abs().mag;
        if a.is_zero() && b.is_zero() {
            return Self::zero();
        }
        while !b.is_zero() {
            let (_, r) = a.div_rem(&b);
            a = b;
            b = r;
        }
        Self { sign: Sign::Positive, mag: a }
    }

    /// 加法。
    pub fn add(&self, rhs: &Self) -> Self {
        match (self.sign, rhs.sign) {
            (Sign::Zero, _) => rhs.clone(),
            (_, Sign::Zero) => self.clone(),
            (Sign::Positive, Sign::Positive) => Self { sign: Sign::Positive, mag: self.mag.add(&rhs.mag) },
            (Sign::Negative, Sign::Negative) => Self { sign: Sign::Negative, mag: self.mag.add(&rhs.mag) },
            (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive) => {
                let sa = &self.mag;
                let sb = &rhs.mag;
                if sa >= sb {
                    let mag = sa.sub(sb);
                    Self::from_mag_sign(mag, self.sign == Sign::Negative)
                }
                else {
                    let mag = sb.sub(sa);
                    Self::from_mag_sign(mag, rhs.sign == Sign::Negative)
                }
            }
        }
    }

    /// 减法。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.neg())
    }

    /// 乘法。
    pub fn mul(&self, rhs: &Self) -> Self {
        if self.is_zero() || rhs.is_zero() {
            return Self::zero();
        }
        let sign = if self.sign == rhs.sign { Sign::Positive } else { Sign::Negative };
        Self { sign, mag: self.mag.mul(&rhs.mag) }
    }

    /// 向零整除商。
    pub fn div(&self, rhs: &Self) -> Self {
        let (q, _) = self.div_rem(rhs);
        q
    }

    /// 向零整除余数。
    pub fn rem(&self, rhs: &Self) -> Self {
        let (_, r) = self.div_rem(rhs);
        r
    }

    fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        assert!(!rhs.is_zero());
        let (q_mag, r_mag) = self.mag.div_rem(&rhs.mag);
        let q_sign = if self.sign == rhs.sign { Sign::Positive } else { Sign::Negative };
        let r_sign = self.sign;
        (Self::from_mag_sign(q_mag, q_sign == Sign::Negative), Self::from_mag_sign(r_mag, r_sign == Sign::Negative))
    }

    /// 模幂：`self^exp mod modulus`（`modulus` 须为正）。
    pub fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        assert!(modulus.is_positive());
        if exp.is_negative() {
            return Self::zero();
        }
        let base = self.rem(modulus).abs();
        let result_mag = base.mag.mod_pow(&exp.abs().mag, &modulus.abs().mag);
        Self { sign: Sign::Positive, mag: result_mag }
    }

    /// 绝对值的二进制位宽（`0` → `0`）。
    pub fn bits(&self) -> u64 {
        self.mag.bits()
    }

    /// 可无损落入 `i64` 时返回。
    pub fn to_i64(&self) -> Option<i64> {
        match self.sign {
            Sign::Zero => Some(0),
            Sign::Positive => self.mag.to_u64().and_then(|u| i64::try_from(u).ok()),
            Sign::Negative => {
                let u = self.mag.to_u64()?;
                if u == i64::MIN.unsigned_abs() {
                    Some(i64::MIN)
                }
                else {
                    i64::try_from(u).ok().map(|v| -v)
                }
            }
        }
    }

    /// 可无损落入 `u64` 时返回。
    pub fn to_u64(&self) -> Option<u64> {
        match self.sign {
            Sign::Zero => Some(0),
            Sign::Positive => self.mag.to_u64(),
            Sign::Negative => None,
        }
    }

    /// 可无损落入 `u128` 时返回。
    pub fn to_u128(&self) -> Option<u128> {
        match self.sign {
            Sign::Zero => Some(0),
            Sign::Positive => self.mag.to_u128(),
            Sign::Negative => None,
        }
    }

    /// IEEE binary64 可精确表示的整数绝对值上限（`2^53`）。
    const F64_EXACT_ABS_MAX: u128 = 1u128 << 53;

    /// 仅当值在 binary64 上可精确表示时返回（可逆）。
    pub fn try_to_f64_exact(&self) -> Option<f64> {
        if self.is_zero() {
            return Some(0.0);
        }
        let u = self.abs().mag.to_u128()?;
        if u > Self::F64_EXACT_ABS_MAX {
            return None;
        }
        let f = if self.is_negative() { -(u as f64) } else { u as f64 };
        if !f.is_finite() {
            return None;
        }
        if f64_represents_integer(f, self) { Some(f) } else { None }
    }

    /// 明确近似的 `f64`（不保证可逆；宿主桥接用）。
    pub fn to_f64_approximate(&self) -> Option<f64> {
        if let Some(i) = self.to_i64() {
            return Some(i as f64);
        }
        self.to_decimal_string().parse::<f64>().ok().filter(|x| x.is_finite())
    }

    /// 同 [`try_to_f64_exact`]（过渡期别名）。
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        self.try_to_f64_exact()
    }

    /// 十进制调试字符串（非本地化用户文案）。
    pub fn to_decimal_string(&self) -> String {
        match self.sign {
            Sign::Zero => "0".to_string(),
            Sign::Positive => self.mag.to_decimal_string(),
            Sign::Negative => format!("-{}", self.mag.to_decimal_string()),
        }
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        self.is_positive() && self.mag.is_power_of_two()
    }

    /// 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && self.mag.is_odd()
    }

    /// 非负 `u32` 指数幂（独立实现，不回调 [`pow`]）。
    pub fn pow_u32(&self, exp: u32) -> Result<Self, ()> {
        if exp as i64 > Self::MAX_POW_EXP {
            return Err(());
        }
        if exp == 0 {
            return Ok(Self::one());
        }
        if self.is_zero() {
            return Ok(Self::zero());
        }
        let mut acc = Self::one();
        let mut base = self.clone();
        let mut e = exp;
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(&base);
            }
            base = base.mul(&base);
            e >>= 1;
        }
        Ok(acc)
    }

    /// 非负整数幂允许的最大指数（与阶乘等资源合同一致）。
    pub const MAX_POW_EXP: i64 = 10_000;

    /// 非负整数幂（二进制幂；指数须 `>= 0` 且 `<= MAX_POW_EXP`）。
    pub fn pow(&self, exp: &Integer) -> Result<Self, ()> {
        if exp.is_negative() {
            return Err(());
        }
        if exp.is_zero() {
            return Ok(Self::one());
        }
        if self.is_zero() {
            return Ok(Self::zero());
        }
        if let Some(e) = exp.to_i64() {
            if e > Self::MAX_POW_EXP {
                return Err(());
            }
        }
        else {
            return Err(());
        }
        let mut acc = Self::one();
        let mut base = self.clone();
        let mut e = exp.clone();
        while !e.is_zero() {
            if e.is_odd() {
                acc = acc.mul(&base);
            }
            base = base.mul(&base);
            e = e.div(&Integer::from_i64(2));
        }
        Ok(acc)
    }

    /// 整数平方根（完全平方时精确；否则向下取整，供 `kernel_number` 检测）。
    pub fn int_sqrt(&self) -> Self {
        if self.is_negative() {
            return Self::zero();
        }
        if self.is_zero() {
            return Self::zero();
        }
        let mut lo = Self::zero();
        let mut hi = self.clone().add(&Self::one());
        while lo.add(&Integer::one()).cmp(&hi) == std::cmp::Ordering::Less {
            let mid = lo.add(&hi).div(&Integer::from_i64(2));
            if mid.mul(&mid) <= *self {
                lo = mid;
            }
            else {
                hi = mid;
            }
        }
        lo
    }
}

fn f64_represents_integer(f: f64, n: &Integer) -> bool {
    if !f.is_finite() {
        return false;
    }
    if n.is_zero() {
        return f == 0.0;
    }
    let u = match n.abs().mag.to_u128() {
        Some(v) => v,
        None => return false,
    };
    let expected = if n.is_negative() { -(u as f64) } else { u as f64 };
    f.to_bits() == expected.to_bits()
}

impl From<i64> for Integer {
    fn from(n: i64) -> Self {
        Self::from_i64(n)
    }
}

impl From<i32> for Integer {
    fn from(n: i32) -> Self {
        Self::from_i64(i64::from(n))
    }
}

impl From<u32> for Integer {
    fn from(n: u32) -> Self {
        Self::from_u64(u64::from(n))
    }
}

impl From<u64> for Integer {
    fn from(n: u64) -> Self {
        Self::from_u64(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div_rem_mod_pow_bits() {
        let a = Integer::from_i64(17);
        let b = Integer::from_i64(5);
        assert_eq!(a.div(&b), Integer::from_i64(3));
        assert_eq!(a.rem(&b), Integer::from_i64(2));
        assert!(a.is_positive());
        assert!(Integer::one().is_one());
        assert_eq!(Integer::from_i64(8).bits(), 4);
        let m = Integer::from_i64(7);
        assert_eq!(Integer::from_i64(3).mod_pow(&Integer::from_i64(4), &m), Integer::from_i64(4));
        assert_eq!(Integer::from_u64(42).to_u64(), Some(42));
        assert_eq!(Integer::from_i64(-1).to_u64(), None);
        let big = Integer::from_decimal_str("99999999999999999999").unwrap();
        assert_eq!(big.to_decimal_string(), "99999999999999999999");
    }

    #[test]
    fn pow_no_recursion_and_large_exponent() {
        let two = Integer::from_i64(2);
        assert_eq!(two.pow_u32(0).unwrap(), Integer::one());
        assert_eq!(two.pow_u32(1).unwrap(), two);
        assert_eq!(two.pow_u32(32).unwrap(), Integer::from_decimal_str("4294967296").unwrap());
        let exp33 = Integer::from_u64(33);
        assert_eq!(two.pow(&exp33).unwrap(), Integer::from_decimal_str("8589934592").unwrap());
        let exp_over = Integer::from_i64(Integer::MAX_POW_EXP + 1);
        assert!(two.pow(&exp_over).is_err());
    }

    #[test]
    fn try_to_f64_exact_respects_2_53() {
        assert_eq!(Integer::from_i64(1).try_to_f64_exact(), Some(1.0));
        let ok = Integer::from_i64(9_007_199_254_740_992); // 2^53
        assert_eq!(ok.try_to_f64_exact(), Some(9_007_199_254_740_992.0));
        let bad = Integer::from_i64(9_007_199_254_740_993);
        assert_eq!(bad.try_to_f64_exact(), None);
        assert!(bad.to_f64_approximate().is_some());
    }

    #[test]
    fn gcd_zero_zero() {
        assert_eq!(Integer::zero().gcd(&Integer::zero()), Integer::zero());
    }
}
