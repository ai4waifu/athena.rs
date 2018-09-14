//! 精确有理数包装。

use num_rational::BigRational;
use num_traits::{One, Zero};

use crate::integer::Integer;

/// 精确有理数（既约，分母为正）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rational {
    inner: BigRational,
}

impl Rational {
    /// 由整数构造。
    pub fn from_integer(n: Integer) -> Self {
        Self {
            inner: BigRational::from_integer(n.as_bigint().clone()),
        }
    }

    /// 分子 / 分母（自动既约）。
    pub fn new(numer: Integer, denom: Integer) -> Self {
        Self {
            inner: BigRational::new(numer.as_bigint().clone(), denom.as_bigint().clone()),
        }
    }

    /// 零。
    pub fn zero() -> Self {
        Self { inner: BigRational::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { inner: BigRational::one() }
    }

    /// 分子。
    pub fn numerator(&self) -> Integer {
        Integer::from_bigint(self.inner.numer().clone())
    }

    /// 分母。
    pub fn denominator(&self) -> Integer {
        Integer::from_bigint(self.inner.denom().clone())
    }
}
