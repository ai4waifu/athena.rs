//! 统一数值值：单一 discriminant，域由 variant 推导；证明元数据见 [`crate::evidence`]。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    algebraic::AlgebraicNumber, big_float::BigFloat, complex::Complex, domain::NumericDomain, finite_field::FiniteFieldValue,
    integer::Integer, interval::Interval, modular::ModularValue, p_adic::PAdicValue, precision::PrecisionInfo,
    rational::Rational, real::Real,
};

/// 带域语义的数值载荷（唯一执行真相源；域与精度由 variant 推导）。
#[derive(Debug, Clone, PartialEq)]
pub enum NumericValue {
    /// 精确整数 ℤ。
    Integer(Integer),
    /// 精确有理 ℚ。
    Rational(Rational),
    /// 实数 ℝ（[`Real::Machine`] 或 [`Real::BigFloat`]）。
    Real(Real),
    /// 复数 ℂ（骨架）。
    Complex(Complex),
    /// 区间（骨架）。
    Interval(Interval),
    /// 代数数（骨架）。
    Algebraic(AlgebraicNumber),
    /// 模整数 ℤ/nℤ（模数在 [`ModularValue`] 内）。
    Modular(ModularValue),
    /// 有限域元素（域 id 在 [`FiniteFieldValue`] 内）。
    FiniteField(FiniteFieldValue),
    /// p-adic（参数在 [`PAdicValue`] 内）。
    PAdic(PAdicValue),
}

/// 过渡别名：公共面统一称 [`NumericValue`]。
pub type Number = NumericValue;

impl NumericValue {
    /// 经验证构造：外部提供的 domain / precision 必须与 variant 一致。
    pub fn try_new(domain: NumericDomain, value: Self, precision: PrecisionInfo) -> Result<Self> {
        if value.domain() != domain {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "numeric_value_validate"));
        }
        if value.precision() != precision {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "numeric_value_validate"));
        }
        value.validate()?;
        Ok(value)
    }

    /// 校验 variant 级不变量（开放构造路径）。
    pub fn validate(&self) -> Result<()> {
        let ok = match self {
            Self::Integer(_) | Self::Rational(_) => true,
            Self::Real(Real::Machine(_)) => true,
            Self::Real(Real::BigFloat(b)) => b.validate().is_ok(),
            Self::Complex(_) => true,
            Self::Modular(_) | Self::FiniteField(_) | Self::Interval(_) | Self::Algebraic(_) => false,
            Self::PAdic(v) => v.validate().is_ok(),
        };
        if ok {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "numeric_value_validate"))
        }
    }

    /// 精确整数。
    pub fn integer(n: Integer) -> Self {
        Self::Integer(n)
    }

    /// 精确有理（既约；分母为 1 时仍保留 Rational 域，除非调用方用 [`Self::from_rational_normalized`]）。
    pub fn rational(r: Rational) -> Self {
        Self::Rational(r)
    }

    /// 有理规范化：分母为 1 时降为 Integer 域。
    pub fn from_rational_normalized(r: Rational) -> Self {
        if r.is_integer() { Self::integer(r.numerator()) } else { Self::rational(r) }
    }

    /// 机器实数。
    pub fn machine_real(x: f64) -> Self {
        Self::Real(Real::machine(x))
    }

    /// 任意精度有限实数。
    pub fn big_float(b: BigFloat) -> Self {
        Self::Real(Real::big_float(b))
    }

    /// 从有限 IEEE binary64 导入任意精度实数（拒绝 NaN/Inf）。
    pub fn big_float_from_f64(x: f64) -> Result<Self> {
        Ok(Self::big_float(BigFloat::from_f64(x)?))
    }

    /// 同 [`Self::machine_real`]。
    pub fn machine(x: f64) -> Self {
        Self::machine_real(x)
    }

    /// 小整数。
    pub fn small_int(n: i64) -> Self {
        Self::integer(Integer::from_i64(n))
    }

    /// `i64` 有理。
    pub fn rational_i64(num: i64, den: i64) -> Result<Self> {
        let r = Rational::try_new(Integer::from_i64(num), Integer::from_i64(den))?;
        Ok(Self::from_rational_normalized(r))
    }

    /// 运算域（由 variant 推导，无第二份副本）。
    pub fn domain(&self) -> NumericDomain {
        match self {
            Self::Integer(_) => NumericDomain::Integer,
            Self::Rational(_) => NumericDomain::Rational,
            Self::Real(_) => NumericDomain::Real,
            Self::Complex(_) => NumericDomain::Complex,
            Self::Interval(_) => NumericDomain::Interval,
            Self::Algebraic(_) => NumericDomain::Algebraic,
            Self::Modular(v) => match v.modulus() {
                Some(m) => NumericDomain::Modular { modulus: m.clone() },
                None => NumericDomain::Integer,
            },
            Self::FiniteField(v) => NumericDomain::FiniteField { field: v.field },
            Self::PAdic(v) => NumericDomain::PAdic { prime: v.prime.clone(), precision: v.precision },
        }
    }

    /// 精度（由 variant 推导）。
    pub fn precision(&self) -> PrecisionInfo {
        match self {
            Self::Integer(_) | Self::Rational(_) | Self::Modular(_) | Self::FiniteField(_) => PrecisionInfo::exact(),
            Self::PAdic(v) => PrecisionInfo::arbitrary(v.precision.saturating_mul(8).max(1)),
            Self::Real(Real::Machine(_)) => PrecisionInfo::machine(),
            Self::Real(Real::BigFloat(b)) => PrecisionInfo::arbitrary(b.precision_bits()),
            Self::Complex(_) | Self::Interval(_) | Self::Algebraic(_) => PrecisionInfo::exact(),
        }
    }

    /// 是否精确零。
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Integer(n) => n.is_zero(),
            Self::Rational(r) => r.is_zero(),
            Self::Real(Real::Machine(x)) => *x == 0.0,
            Self::Real(Real::BigFloat(b)) => b.is_zero(),
            _ => false,
        }
    }

    /// 是否精确一。
    pub fn is_one(&self) -> bool {
        match self {
            Self::Integer(n) => n.is_one(),
            Self::Rational(r) => r.is_integer() && r.numerator().is_one(),
            Self::Real(Real::Machine(x)) => *x == 1.0,
            Self::Real(Real::BigFloat(b)) => b.is_one(),
            _ => false,
        }
    }

    /// 是否精确 `-1`。
    pub fn is_neg_one(&self) -> bool {
        match self {
            Self::Integer(n) => n.to_i64() == Some(-1),
            Self::Rational(r) => r.is_integer() && r.numerator().to_i64() == Some(-1),
            Self::Real(Real::Machine(x)) => *x == -1.0,
            Self::Real(Real::BigFloat(b)) => b.sign() == crate::integer::Sign::Negative && b.is_one(),
            _ => false,
        }
    }

    /// 逻辑真值（精确非零 → true；NaN → false）。
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Integer(n) => !n.is_zero(),
            Self::Rational(r) => !r.is_zero(),
            Self::Real(Real::Machine(x)) => *x != 0.0 && !x.is_nan(),
            Self::Real(Real::BigFloat(b)) => !b.is_zero(),
            _ => false,
        }
    }

    /// 可落入 `i64` 的整数指数。
    pub fn as_integer_exp(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => n.to_i64(),
            Self::Rational(r) if r.is_integer() => r.numerator().to_i64(),
            _ => None,
        }
    }

    /// 同 [`Self::as_integer_exp`]。
    pub fn as_exact_integer(&self) -> Option<i64> {
        self.as_integer_exp()
    }

    /// 机器 `f64`（仅 Machine Real）。
    pub fn as_machine_f64(&self) -> Option<f64> {
        match self {
            Self::Real(Real::Machine(x)) => Some(*x),
            _ => None,
        }
    }

    /// 渲染字符串。
    pub fn to_render_string(&self) -> String {
        match self {
            Self::Integer(n) => n.to_decimal_string(),
            Self::Rational(r) => r.to_wire_string(),
            Self::Real(Real::Machine(x)) => {
                if x.fract() == 0.0 && x.abs() < 1e15 {
                    format!("{}", *x as i64)
                }
                else {
                    format!("{x}")
                }
            }
            Self::Real(Real::BigFloat(b)) => {
                if let Some(x) = b.to_f64_exact() {
                    if x.fract() == 0.0 && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x}") }
                }
                else {
                    format!("{b:?}")
                }
            }
            other => format!("{other:?}"),
        }
    }

    /// 整数视图。
    pub fn as_integer(&self) -> Option<&Integer> {
        match self {
            Self::Integer(n) => Some(n),
            _ => None,
        }
    }

    /// 有理视图。
    pub fn as_rational(&self) -> Option<&Rational> {
        match self {
            Self::Rational(r) => Some(r),
            _ => None,
        }
    }

    /// 实数视图。
    pub fn as_real(&self) -> Option<&Real> {
        match self {
            Self::Real(r) => Some(r),
            _ => None,
        }
    }
}
