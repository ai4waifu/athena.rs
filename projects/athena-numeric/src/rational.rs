//! 精确有理数包装（纯 Rust；分子 / 分母为 [`Integer`]）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::integer::{Integer, Sign};

/// 精确有理数（既约，分母为正；亦称 [`ExactRational`]）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rational {
    numer: Integer,
    denom: Integer,
}

/// Living `16` 稳定别名。
pub type ExactRational = Rational;

impl Rational {
    /// 由整数构造。
    pub fn from_integer(n: Integer) -> Self {
        Self { numer: n, denom: Integer::one() }
    }

    /// 分子 / 分母（自动既约；分母为零失败）。
    pub fn try_new(numer: Integer, denom: Integer) -> Result<Self, Diagnostic> {
        if denom.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_new"));
        }
        Ok(Self::normalize_pair(numer, denom))
    }

    /// 分子 / 分母（自动既约）。分母为零时 panic——优先 [`try_new`].
    pub fn new(numer: Integer, denom: Integer) -> Self {
        Self::try_new(numer, denom).expect("rational denominator must be non-zero")
    }

    fn normalize_pair(numer: Integer, denom: Integer) -> Self {
        if denom.is_zero() {
            return Self { numer: Integer::zero(), denom: Integer::one() };
        }
        let g = numer.abs().gcd(&denom.abs());
        let mut n = if g.is_one() { numer } else { numer.div(&g) };
        let mut d = if g.is_one() { denom } else { denom.div(&g) };
        if d.is_negative() {
            n = n.neg();
            d = d.neg();
        }
        if d.is_one() { Self { numer: n, denom: Integer::one() } } else { Self { numer: n, denom: d } }
    }

    /// 显式规范化（分母为正、既约）。
    pub fn normalize(self) -> Self {
        Self::normalize_pair(self.numer, self.denom)
    }

    /// 零。
    pub fn zero() -> Self {
        Self { numer: Integer::zero(), denom: Integer::one() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { numer: Integer::one(), denom: Integer::one() }
    }

    /// 分子。
    pub fn numerator(&self) -> Integer {
        if self.denom.is_one() && !self.numer.is_zero() {
            self.numer.clone()
        }
        else if self.numer.is_zero() {
            Integer::zero()
        }
        else {
            self.numer.clone()
        }
    }

    /// 分母（恒正；整数时为 1）。
    pub fn denominator(&self) -> Integer {
        if self.numer.is_zero() { Integer::one() } else { self.denom.clone() }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.numer.is_zero()
    }

    /// 是否为负。
    pub fn is_negative(&self) -> bool {
        self.numer.is_negative()
    }

    /// 是否非负（零视为非负）。
    pub fn is_non_negative(&self) -> bool {
        !self.numer.is_negative()
    }

    /// 是否为整数（分母为 1）。
    pub fn is_integer(&self) -> bool {
        self.denom.is_one()
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        self.numer.sign()
    }

    /// 绝对值。
    pub fn abs(&self) -> Self {
        Self { numer: self.numer.abs(), denom: self.denom.clone() }
    }

    /// 取负。
    pub fn neg(&self) -> Self {
        Self { numer: self.numer.neg(), denom: self.denom.clone() }
    }

    /// 加法。
    pub fn add(&self, rhs: &Self) -> Self {
        let n = self.numer.mul(&rhs.denom).add(&rhs.numer.mul(&self.denom));
        let d = self.denom.mul(&rhs.denom);
        Self::normalize_pair(n, d)
    }

    /// 减法。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.neg())
    }

    /// 乘法。
    pub fn mul(&self, rhs: &Self) -> Self {
        Self::normalize_pair(self.numer.mul(&rhs.numer), self.denom.mul(&rhs.denom))
    }

    /// 除法。
    pub fn try_div(&self, rhs: &Self) -> Result<Self, Diagnostic> {
        if rhs.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_div"));
        }
        Ok(Self::normalize_pair(self.numer.mul(&rhs.denom), self.denom.mul(&rhs.numer)))
    }

    /// 非负整数幂。
    pub fn pow_u32(&self, exp: u32) -> Result<Self, Diagnostic> {
        if exp == 0 {
            return Ok(Self::one());
        }
        let n = self.numer.pow_u32(exp).map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
        let d = self.denom.pow_u32(exp).map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
        Ok(Self::normalize_pair(n, d))
    }

    /// 仅当值在 binary64 上可精确表示时返回。
    pub fn try_to_f64_exact(&self) -> Option<f64> {
        if self.is_integer() {
            return self.numerator().try_to_f64_exact();
        }
        let d = self.denominator();
        if !d.is_power_of_two() {
            return None;
        }
        let nf = self.numerator().try_to_f64_exact()?;
        let df = d.try_to_f64_exact()?;
        if df == 0.0 {
            return None;
        }
        let q = nf / df;
        if !q.is_finite() {
            return None;
        }
        if nf.to_bits() == (q * df).to_bits() { Some(q) } else { None }
    }

    /// 明确近似的 `f64`。
    pub fn to_f64_approximate(&self) -> Option<f64> {
        let nf = self.numerator().to_f64_approximate()?;
        let df = self.denominator().to_f64_approximate()?;
        if df == 0.0 {
            return None;
        }
        let q = nf / df;
        if q.is_finite() { Some(q) } else { None }
    }

    /// 同 [`try_to_f64_exact`]（过渡期别名）。
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        self.try_to_f64_exact()
    }

    /// `numer/denom` 十进制载荷。
    pub fn to_wire_string(&self) -> String {
        if self.denom.is_one() {
            self.numer.to_decimal_string()
        }
        else {
            format!("{}/{}", self.numer.to_decimal_string(), self.denom.to_decimal_string())
        }
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
