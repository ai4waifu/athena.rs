//! 精确整数包装（内部可换 backend，不暴露 `num_bigint`）。

use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

/// 精确整数。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integer {
    inner: BigInt,
}

impl Integer {
    /// 由已解码 `i64` 构造。
    pub fn from_i64(n: i64) -> Self {
        Self { inner: BigInt::from(n) }
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

    /// 绝对值。
    pub fn abs(&self) -> Self {
        Self { inner: self.inner.abs() }
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
