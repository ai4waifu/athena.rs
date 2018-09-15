//! 精确整数包装（内部可换 backend，不暴露 `num_bigint`）。

use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

/// 符号（精确）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    inner: BigInt,
}

/// Living `16` 稳定别名。
pub type ExactInteger = Integer;

impl Integer {
    /// 由已解码 `i64` 构造。
    pub fn from_i64(n: i64) -> Self {
        Self { inner: BigInt::from(n) }
    }

    /// 解析十进制字符串（可含前导 `+/-`）。
    pub fn from_decimal_str(s: &str) -> Result<Self, ()> {
        s.parse::<BigInt>().map(|inner| Self { inner }).map_err(|_| ())
    }

    /// 零。
    pub fn zero() -> Self {
        Self { inner: BigInt::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { inner: BigInt::one() }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 是否为负。
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        if self.inner.is_zero() {
            Sign::Zero
        }
        else if self.inner.is_negative() {
            Sign::Negative
        }
        else {
            Sign::Positive
        }
    }

    /// 绝对值。
    pub fn abs(&self) -> Self {
        Self { inner: self.inner.abs() }
    }

    /// 取负。
    pub fn neg(&self) -> Self {
        Self { inner: -&self.inner }
    }

    /// 非负最大公约数。
    pub fn gcd(&self, other: &Self) -> Self {
        let mut a = self.inner.abs();
        let mut b = other.inner.abs();
        while !b.is_zero() {
            let r = &a % &b;
            a = b;
            b = r;
        }
        Self { inner: a }
    }

    /// 加法。
    pub fn add(&self, rhs: &Self) -> Self {
        Self { inner: &self.inner + &rhs.inner }
    }

    /// 减法。
    pub fn sub(&self, rhs: &Self) -> Self {
        Self { inner: &self.inner - &rhs.inner }
    }

    /// 乘法。
    pub fn mul(&self, rhs: &Self) -> Self {
        Self { inner: &self.inner * &rhs.inner }
    }

    /// 可无损落入 `i64` 时返回。
    pub fn to_i64(&self) -> Option<i64> {
        use num_traits::ToPrimitive;
        self.inner.to_i64()
    }

    /// 可无损落入有限 `f64` 时返回（大整数可能失败）。
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        use num_traits::ToPrimitive;
        let x = self.inner.to_f64()?;
        if x.is_finite() { Some(x) } else { None }
    }

    /// 内部存储（crate 内迁移用；非稳定公共 API）。
    pub(crate) fn as_bigint(&self) -> &BigInt {
        &self.inner
    }

    /// 由内部 bigint 构造（crate 内）。
    pub(crate) fn from_bigint(inner: BigInt) -> Self {
        Self { inner }
    }

    /// 十进制调试字符串（非本地化用户文案）。
    pub fn to_decimal_string(&self) -> String {
        self.inner.to_string()
    }
}

impl From<i64> for Integer {
    fn from(n: i64) -> Self {
        Self::from_i64(n)
    }
}

impl From<u32> for Integer {
    fn from(n: u32) -> Self {
        Self { inner: BigInt::from(n) }
    }
}
