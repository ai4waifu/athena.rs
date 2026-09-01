//! 精确有理数包装。

use athena_types::{Diagnostic, DiagnosticCode};
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::integer::{Integer, Sign};

/// 精确有理数（既约，分母为正；亦称 [`ExactRational`]）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rational {
    inner: BigRational,
}

/// Living `16` 稳定别名。
pub type ExactRational = Rational;

impl Rational {
    /// 由整数构造。
    pub fn from_integer(n: Integer) -> Self {
        Self { inner: BigRational::from_integer(n.as_bigint().clone()) }
    }

    /// 分子 / 分母（自动既约；分母为零失败）。
    pub fn try_new(numer: Integer, denom: Integer) -> Result<Self, Diagnostic> {
        if denom.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_new"));
        }
        Ok(Self::new_unchecked(numer, denom))
    }

    /// 分子 / 分母（自动既约）。分母为零时 panic——优先 [`try_new`]。
    pub fn new(numer: Integer, denom: Integer) -> Self {
        Self::new_unchecked(numer, denom)
    }

    fn new_unchecked(numer: Integer, denom: Integer) -> Self {
        Self { inner: BigRational::new(numer.as_bigint().clone(), denom.as_bigint().clone()) }
    }

    /// 显式规范化（`new` / `try_new` 已既约；此方法保证分母为正的既约形）。
    pub fn normalize(self) -> Self {
        let n = self.inner.numer().clone();
        let d = self.inner.denom().clone();
        Self { inner: BigRational::new(n, d) }
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

    /// 分母（恒正）。
    pub fn denominator(&self) -> Integer {
        Integer::from_bigint(self.inner.denom().clone())
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 是否为整数（分母为 1）。
    pub fn is_integer(&self) -> bool {
        self.inner.denom().is_one()
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

    /// 除法。
    pub fn try_div(&self, rhs: &Self) -> Result<Self, Diagnostic> {
        if rhs.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_div"));
        }
        Ok(Self { inner: &self.inner / &rhs.inner })
    }

    /// 可无损落入有限 `f64` 时返回。
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        use num_traits::ToPrimitive;
        let x = self.inner.to_f64()?;
        if x.is_finite() { Some(x) } else { None }
    }

    /// `numer/denom` 十进制载荷。
    pub fn to_wire_string(&self) -> String {
        format!("{}/{}", self.numerator().to_decimal_string(), self.denominator().to_decimal_string())
    }

    /// 解析 `numer/denom` 或纯整数十进制。
    pub fn from_wire_string(s: &str) -> Result<Self, Diagnostic> {
        if let Some((n, d)) = s.split_once('/') {
            let numer = Integer::from_decimal_str(n).map_err(|_| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "rational_from_wire")
            })?;
            let denom = Integer::from_decimal_str(d).map_err(|_| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "rational_from_wire")
            })?;
            Self::try_new(numer, denom)
        }
        else {
            let n = Integer::from_decimal_str(s).map_err(|_| {
                Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                    .detail("domain", "numeric")
                    .detail("operation", "rational_from_wire")
            })?;
            Ok(Self::from_integer(n))
        }
    }
}
