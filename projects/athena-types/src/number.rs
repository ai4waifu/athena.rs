//! 数字类型定义（合同层，不含求值算法）。

use num_traits::{One, Signed};

/// 精确整数或既约有理数（分母为正）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactNumber {
    /// 任意精度整数。
    Integer(num_bigint::BigInt),
    /// 既约有理数。
    Rational(num_rational::BigRational),
}

impl ExactNumber {
    /// Construct integer.
    pub fn integer(n: impl Into<num_bigint::BigInt>) -> Self {
        Self::Integer(n.into())
    }

    /// Construct normalized rational.
    pub fn rational(num: impl Into<num_bigint::BigInt>, den: impl Into<num_bigint::BigInt>) -> Self {
        let r = num_rational::BigRational::new(num.into(), den.into());
        normalize_rational(r)
    }

    /// From reduced `BigRational`.
    pub fn from_rational(r: num_rational::BigRational) -> Self {
        normalize_rational(r)
    }

    /// Whether exactly zero.
    pub fn is_zero(&self) -> bool {
        use num_traits::Zero;
        match self {
            Self::Integer(n) => n.is_zero(),
            Self::Rational(r) => r.is_zero(),
        }
    }

    /// Whether exactly one.
    pub fn is_one(&self) -> bool {
        use num_traits::One;
        match self {
            Self::Integer(n) => n.is_one(),
            Self::Rational(r) => r.is_one(),
        }
    }

    /// Whether exactly `-1`.
    pub fn is_neg_one(&self) -> bool {
        match self {
            Self::Integer(n) => n == &num_bigint::BigInt::from(-1),
            Self::Rational(r) => *r == num_rational::BigRational::from_integer((-1).into()),
        }
    }

    /// Integer exponent when representable as `BigInt`.
    pub fn as_integer_exp(&self) -> Option<num_bigint::BigInt> {
        match self {
            Self::Integer(n) => Some(n.clone()),
            Self::Rational(r) if r.denom().is_one() => Some(r.numer().clone()),
            _ => None,
        }
    }
}

/// Inexact real storage (phase 1: machine float only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RealNumber {
    /// IEEE-754 binary64.
    Machine(f64),
}

impl RealNumber {
    /// Whether zero.
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Machine(n) if *n == 0.0)
    }

    /// Whether one.
    pub fn is_one(&self) -> bool {
        matches!(self, Self::Machine(n) if *n == 1.0)
    }

    /// Whether `-1`.
    pub fn is_neg_one(&self) -> bool {
        matches!(self, Self::Machine(n) if *n == -1.0)
    }
}

/// Unified kernel number (wire-stable representation).
#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    /// Exact integer or rational.
    Exact(ExactNumber),
    /// Inexact real.
    Real(RealNumber),
}

impl Number {
    /// Exact integer.
    pub fn integer(n: impl Into<num_bigint::BigInt>) -> Self {
        Self::Exact(ExactNumber::Integer(n.into()))
    }

    /// Exact rational (normalized).
    pub fn rational(r: num_rational::BigRational) -> Self {
        Self::Exact(normalize_rational(r))
    }

    /// Machine real.
    pub fn machine(n: f64) -> Self {
        Self::Real(RealNumber::Machine(n))
    }

    /// Small `i64` convenience.
    pub fn small_int(n: i64) -> Self {
        Self::integer(num_bigint::BigInt::from(n))
    }

    /// Whether exactly zero.
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Exact(e) => e.is_zero(),
            Self::Real(r) => r.is_zero(),
        }
    }

    /// Whether exactly one.
    pub fn is_one(&self) -> bool {
        match self {
            Self::Exact(e) => e.is_one(),
            Self::Real(r) => r.is_one(),
        }
    }

    /// Whether exactly `-1`.
    pub fn is_neg_one(&self) -> bool {
        match self {
            Self::Exact(e) => e.is_neg_one(),
            Self::Real(r) => r.is_neg_one(),
        }
    }

    /// Truthiness for logic (exact non-zero → true; NaN → false).
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Exact(e) => !e.is_zero(),
            Self::Real(RealNumber::Machine(n)) => *n != 0.0 && !n.is_nan(),
        }
    }

    /// Integer exponent when representable.
    pub fn as_integer_exp(&self) -> Option<num_bigint::BigInt> {
        match self {
            Self::Exact(e) => e.as_integer_exp(),
            _ => None,
        }
    }

    /// Exact integer if representable.
    pub fn as_exact_integer(&self) -> Option<num_bigint::BigInt> {
        match self {
            Self::Exact(ExactNumber::Integer(n)) => Some(n.clone()),
            Self::Exact(ExactNumber::Rational(r)) if r.denom().is_one() => Some(r.numer().clone()),
            _ => None,
        }
    }

    /// Lossy `f64` — host bridge only, not semantic evaluation.
    pub fn to_f64_lossy(&self) -> Option<f64> {
        use num_traits::ToPrimitive;
        match self {
            Self::Exact(ExactNumber::Integer(n)) => n.to_f64(),
            Self::Exact(ExactNumber::Rational(r)) => r.to_f64(),
            Self::Real(RealNumber::Machine(n)) => Some(*n),
        }
    }

    /// Compare when defined.
    pub fn compare(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use num_traits::ToPrimitive;
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => Some(exact_to_rational(a).cmp(&exact_to_rational(b))),
            (Self::Real(RealNumber::Machine(a)), Self::Real(RealNumber::Machine(b))) => a.partial_cmp(b),
            (Self::Exact(a), Self::Real(RealNumber::Machine(b))) => exact_to_rational(a).to_f64()?.partial_cmp(b),
            (Self::Real(RealNumber::Machine(a)), Self::Exact(b)) => a.partial_cmp(&exact_to_rational(b).to_f64()?),
        }
    }

    /// Addition with promotion.
    pub fn add(self, other: Self) -> crate::Result<Self> {
        use crate::{Diagnostic, DiagnosticCode};
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => Ok(Self::Exact(add_exact(a, b))),
            (a, b) => {
                let x = a.to_f64_lossy().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed))?;
                let y = b.to_f64_lossy().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed))?;
                Ok(Self::Real(RealNumber::Machine(x + y)))
            }
        }
    }

    /// Multiplication with promotion.
    pub fn mul(self, other: Self) -> crate::Result<Self> {
        use crate::{Diagnostic, DiagnosticCode};
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => Ok(Self::Exact(mul_exact(a, b))),
            (a, b) => {
                let x = a.to_f64_lossy().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed))?;
                let y = b.to_f64_lossy().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed))?;
                Ok(Self::Real(RealNumber::Machine(x * y)))
            }
        }
    }

    /// Division; zero denominator → `athena_DIVIDE_BY_ZERO`.
    pub fn div(self, other: Self) -> crate::Result<Self> {
        use crate::{Diagnostic, DiagnosticCode};
        if other.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero));
        }
        match (self, other) {
            (Self::Exact(a), Self::Exact(b)) => {
                Ok(Self::Exact(normalize_rational(exact_to_rational(&a) / exact_to_rational(&b))))
            }
            (a, b) => {
                let x = a.to_f64_lossy().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed))?;
                let y = b.to_f64_lossy().ok_or_else(|| Diagnostic::new(DiagnosticCode::PromotionFailed))?;
                Ok(Self::Real(RealNumber::Machine(x / y)))
            }
        }
    }

    /// Power.
    pub fn pow(&self, exp: &Number) -> crate::Result<Self> {
        use crate::{Diagnostic, DiagnosticCode};
        use num_traits::{Pow, Signed, ToPrimitive};
        if exp.is_zero() {
            return Ok(Self::small_int(1));
        }
        if exp.is_one() {
            return Ok(self.clone());
        }
        if exp.is_neg_one() {
            return Self::small_int(1).div(self.clone());
        }
        match (self, exp) {
            (Self::Exact(base), Self::Exact(ExactNumber::Integer(e))) if !e.is_negative() => match base {
                ExactNumber::Integer(n) => Ok(Self::Exact(ExactNumber::Integer(pow_bigint(n, e)?))),
                ExactNumber::Rational(r) => {
                    let exp = u32::try_from(e).map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
                    Ok(Self::Exact(normalize_rational(Pow::pow(r, exp))))
                }
            },
            (Self::Exact(_base), Self::Exact(exp_e)) => {
                if let Some(e) = exp_e.as_integer_exp() {
                    if e.is_negative() {
                        let pos = e.abs();
                        let v = self.pow(&Self::Exact(ExactNumber::Integer(pos)))?;
                        return Self::small_int(1).div(v);
                    }
                    return self.pow(&Self::Exact(ExactNumber::Integer(e)));
                }
                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
            }
            (Self::Real(RealNumber::Machine(b)), Self::Exact(ExactNumber::Integer(e))) => {
                let ef = e.to_f64().ok_or_else(|| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
                Ok(Self::Real(RealNumber::Machine(b.powf(ef))))
            }
            (Self::Real(RealNumber::Machine(b)), Self::Real(RealNumber::Machine(e))) => {
                Ok(Self::Real(RealNumber::Machine(b.powf(*e))))
            }
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)),
        }
    }

    /// Factorial for non-negative exact integers.
    pub fn factorial(&self) -> crate::Result<Self> {
        use crate::{Diagnostic, DiagnosticCode};
        use num_traits::One;
        let n = match self {
            Self::Exact(ExactNumber::Integer(n)) if n.is_negative() => {
                return Err(Diagnostic::new(DiagnosticCode::DomainError));
            }
            Self::Exact(ExactNumber::Integer(n)) => n.clone(),
            Self::Exact(ExactNumber::Rational(r)) if r.denom().is_one() && !r.numer().is_negative() => r.numer().clone(),
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch));
            }
        };
        if n > 10_000u32.into() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError));
        }
        let mut acc = num_bigint::BigInt::one();
        let mut i = num_bigint::BigInt::from(2);
        while i <= n {
            acc *= &i;
            i += 1;
        }
        Ok(Self::integer(acc))
    }

    /// Square root when exact perfect square or machine.
    pub fn sqrt(&self) -> crate::Result<Option<Self>> {
        use num_rational::BigRational;
        use num_traits::Zero;
        Ok(match self {
            Self::Exact(ExactNumber::Integer(n)) if n.is_negative() => None,
            Self::Exact(ExactNumber::Integer(n)) => {
                let root = int_sqrt(n);
                if &root * &root == *n { Some(Self::integer(root)) } else { None }
            }
            Self::Exact(ExactNumber::Rational(r)) if r >= &BigRational::zero() => {
                let num = int_sqrt(r.numer());
                let den = int_sqrt(r.denom());
                if &num * &num == *r.numer() && &den * &den == *r.denom() {
                    Some(Self::Exact(ExactNumber::Rational(BigRational::new(num, den))))
                }
                else {
                    None
                }
            }
            Self::Real(RealNumber::Machine(n)) if *n >= 0.0 => Some(Self::Real(RealNumber::Machine(n.sqrt()))),
            Self::Real(RealNumber::Machine(_)) => None,
            _ => None,
        })
    }

    /// Absolute value.
    pub fn abs(self) -> Self {
        use num_traits::Signed;
        match self {
            Self::Exact(ExactNumber::Integer(n)) => Self::integer(n.abs()),
            Self::Exact(ExactNumber::Rational(r)) => Self::Exact(ExactNumber::Rational(r.abs())),
            Self::Real(RealNumber::Machine(n)) => Self::Real(RealNumber::Machine(n.abs())),
        }
    }

    /// Negation.
    pub fn neg(self) -> Self {
        match self {
            Self::Exact(ExactNumber::Integer(n)) => Self::integer(-n),
            Self::Exact(ExactNumber::Rational(r)) => Self::Exact(ExactNumber::Rational(-r)),
            Self::Real(RealNumber::Machine(n)) => Self::Real(RealNumber::Machine(-n)),
        }
    }
}

fn exact_to_rational(e: &ExactNumber) -> num_rational::BigRational {
    match e {
        ExactNumber::Integer(n) => num_rational::BigRational::from_integer(n.clone()),
        ExactNumber::Rational(r) => r.clone(),
    }
}

fn add_exact(a: ExactNumber, b: ExactNumber) -> ExactNumber {
    normalize_rational(exact_to_rational(&a) + exact_to_rational(&b))
}

fn mul_exact(a: ExactNumber, b: ExactNumber) -> ExactNumber {
    normalize_rational(exact_to_rational(&a) * exact_to_rational(&b))
}

fn int_sqrt(n: &num_bigint::BigInt) -> num_bigint::BigInt {
    use num_traits::{Signed, Zero};
    if n.is_negative() {
        return num_bigint::BigInt::zero();
    }
    n.to_biguint().map(|u| u.sqrt().into()).unwrap_or_else(num_bigint::BigInt::zero)
}

fn pow_bigint(n: &num_bigint::BigInt, e: &num_bigint::BigInt) -> crate::Result<num_bigint::BigInt> {
    use crate::{Diagnostic, DiagnosticCode};
    use num_traits::{Pow, Signed};
    if e.is_negative() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError));
    }
    if let Ok(u) = u32::try_from(e) {
        return Ok(n.pow(u));
    }
    if let Some(bu) = e.to_biguint() {
        return Ok(Pow::pow(n, bu));
    }
    Err(Diagnostic::new(DiagnosticCode::ExponentOutOfRange))
}

/// Normalize rational to integer when denominator is one.
pub fn normalize_rational(r: num_rational::BigRational) -> ExactNumber {
    if r.denom().is_one() { ExactNumber::Integer(r.numer().clone()) } else { ExactNumber::Rational(r) }
}
