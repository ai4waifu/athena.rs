//! 统一数值值：单一 discriminant，域由 variant 推导；证明元数据见 [`crate::evidence`]。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    algebraic::AlgebraicNumber,
    complex::Complex,
    decimal::Decimal,
    domain::NumericDomain,
    finite_field::FiniteFieldValue,
    integer::{Integer, Sign},
    interval::Interval,
    modular::ModularValue,
    p_adic::PAdicValue,
    precision::PrecisionInfo,
    rational::Rational,
    real::Real,
};

/// 带域语义的数值载荷（唯一执行真相源；域与精度由 variant 推导）。
///
/// Living `19`：**不** derive [`Clone`]。用 [`Self::clone_inline`] / [`Self::try_clone_in`]。
#[derive(Debug, PartialEq)]
pub enum NumericValue {
    /// 精确整数 ℤ。
    Integer(Integer),
    /// 精确有理 ℚ。
    Rational(Rational),
    /// 实数 ℝ（[`Real::Machine`] 或 [`Real::Decimal`]）。
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
            Self::Real(Real::Decimal(b)) => b.validate().is_ok(),
            Self::Complex(z) => z.validate().is_ok(),
            Self::Interval(_) => true,
            Self::Modular(v) => v.modulus().is_some(),
            Self::FiniteField(v) => v.validate().is_ok(),
            Self::Algebraic(a) => a.validate().is_ok(),
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

    /// 实数（[`Real::Machine`] 或 [`Real::Decimal`]）。
    pub fn real(r: Real) -> Self {
        Self::Real(r)
    }

    /// 复数。
    pub fn complex(z: Complex) -> Self {
        Self::Complex(z)
    }

    /// 区间。
    pub fn interval(i: Interval) -> Self {
        Self::Interval(i)
    }

    /// 模整数（须嵌入模数方可经 ANV1 往返）。
    pub fn modular(m: ModularValue) -> Self {
        Self::Modular(m)
    }

    /// 代数数。
    pub fn algebraic(a: AlgebraicNumber) -> Self {
        Self::Algebraic(a)
    }

    /// 有限域元素。
    pub fn finite_field(v: FiniteFieldValue) -> Self {
        Self::FiniteField(v)
    }

    /// p-adic 截断值。
    pub fn padic(v: PAdicValue) -> Self {
        Self::PAdic(v)
    }

    /// 机器实数。
    pub fn machine_real(x: f64) -> Self {
        Self::Real(Real::machine(x))
    }

    /// 任意精度有限实数。
    pub fn decimal(b: Decimal) -> Self {
        Self::Real(Real::decimal(b))
    }

    /// 从有限 IEEE binary64 导入任意精度实数（拒绝 NaN/Inf）。
    pub fn decimal_from_f64(x: f64) -> Result<Self> {
        Ok(Self::decimal(Decimal::from_f64(x)?))
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
                Some(m) => NumericDomain::Modular {
                    modulus: m
                        .try_clone_in(&crate::policy::execution_budget::NumericContext::portable_default())
                        .expect("portable default unbounded"),
                },
                None => NumericDomain::Integer,
            },
            Self::FiniteField(v) => NumericDomain::FiniteField { field: v.field() },
            Self::PAdic(v) => NumericDomain::PAdic {
                prime: v
                    .prime
                    .try_clone_in(&crate::policy::execution_budget::NumericContext::portable_default())
                    .expect("portable default unbounded"),
                precision: v.precision,
            },
        }
    }

    /// 精度（由 variant 推导）。
    pub fn precision(&self) -> PrecisionInfo {
        match self {
            Self::Integer(_) | Self::Rational(_) | Self::Modular(_) | Self::FiniteField(_) => PrecisionInfo::exact(),
            Self::PAdic(v) => PrecisionInfo::arbitrary(v.precision.saturating_mul(8).max(1)),
            Self::Real(Real::Machine(_)) => PrecisionInfo::machine(),
            Self::Real(Real::Decimal(b)) => PrecisionInfo::arbitrary(b.precision_bits()),
            Self::Complex(_) | Self::Interval(_) | Self::Algebraic(_) => PrecisionInfo::exact(),
        }
    }

    /// 是否精确零。
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Integer(n) => n.is_zero(),
            Self::Rational(r) => r.is_zero(),
            Self::Real(Real::Machine(x)) => *x == 0.0,
            Self::Real(Real::Decimal(b)) => b.is_zero(),
            _ => false,
        }
    }

    /// 是否精确一。
    pub fn is_one(&self) -> bool {
        match self {
            Self::Integer(n) => n.is_one(),
            Self::Rational(r) => r.is_integer() && r.numerator().is_one(),
            Self::Real(Real::Machine(x)) => *x == 1.0,
            Self::Real(Real::Decimal(b)) => b.is_one(),
            _ => false,
        }
    }

    /// 是否精确 `-1`。
    pub fn is_neg_one(&self) -> bool {
        match self {
            Self::Integer(n) => n.to_i64() == Some(-1),
            Self::Rational(r) => r.is_integer() && r.numerator().to_i64() == Some(-1),
            Self::Real(Real::Machine(x)) => *x == -1.0,
            Self::Real(Real::Decimal(b)) => b.sign() == crate::value::integer::Sign::Negative && b.is_one(),
            _ => false,
        }
    }

    /// 逻辑真值（精确非零 → true；NaN → false）。
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Integer(n) => !n.is_zero(),
            Self::Rational(r) => !r.is_zero(),
            Self::Real(Real::Machine(x)) => *x != 0.0 && !x.is_nan(),
            Self::Real(Real::Decimal(b)) => !b.is_zero(),
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
            Self::Real(Real::Decimal(b)) => {
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

    /// Stable domain/kind tag for IR fingerprints (never `Debug` formatting).
    pub fn fingerprint_domain_tag(&self) -> u64 {
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let mix = |h: u64, v: u64| h.wrapping_mul(FNV_PRIME).wrapping_add(v);
        match self {
            Self::Integer(_) => 1,
            Self::Rational(_) => 2,
            Self::Real(Real::Machine(_)) => 3,
            Self::Real(Real::Decimal(_)) => 4,
            Self::Complex(_) => 5,
            Self::Interval(_) => 6,
            Self::Algebraic(_) => 7,
            Self::Modular(v) => {
                let mut h = 8u64;
                if let Some(m) = v.modulus() {
                    for &limb in m.as_limbs() {
                        h = mix(h, limb);
                    }
                }
                h
            }
            Self::FiniteField(v) => mix(9, u64::from(v.field().0)),
            Self::PAdic(v) => {
                let mut h = mix(10, u64::from(v.precision));
                for b in v.prime.to_decimal_string().as_bytes() {
                    h = mix(h, u64::from(*b));
                }
                h
            }
        }
    }

    /// Binary-stable content hash for IR fingerprints (Integer/Rational limb wire; no render string).
    ///
    /// Other variants still mix a domain tag and a best-effort payload; full ANV1 wire coverage is later.
    pub fn fingerprint_content_hash(&self) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let mix = |h: &mut u64, v: u64| {
            *h ^= v;
            *h = h.wrapping_mul(FNV_PRIME);
        };
        let mix_integer = |h: &mut u64, n: &Integer| {
            let sign_code = match n.sign() {
                Sign::Zero => 0u64,
                Sign::Positive => 1,
                Sign::Negative => 2,
            };
            mix(h, sign_code);
            mix(h, n.as_limbs().len() as u64);
            for &limb in n.as_limbs() {
                mix(h, limb);
            }
        };

        let mut h = FNV_OFFSET;
        mix(&mut h, self.fingerprint_domain_tag());
        match self {
            Self::Integer(n) => mix_integer(&mut h, n),
            Self::Rational(r) => {
                mix_integer(&mut h, &r.numerator());
                mix_integer(&mut h, &r.denominator());
            }
            Self::Real(Real::Machine(x)) => mix(&mut h, x.to_bits()),
            other => {
                for b in other.to_render_string().as_bytes() {
                    mix(&mut h, u64::from(*b));
                }
            }
        }
        h
    }

    /// 代数数视图。
    pub fn as_algebraic(&self) -> Option<&AlgebraicNumber> {
        match self {
            Self::Algebraic(a) => Some(a),
            _ => None,
        }
    }

    /// 有限域元素视图。
    pub fn as_finite_field(&self) -> Option<&FiniteFieldValue> {
        match self {
            Self::FiniteField(v) => Some(v),
            _ => None,
        }
    }

    /// p-adic 视图。
    pub fn as_padic(&self) -> Option<&PAdicValue> {
        match self {
            Self::PAdic(v) => Some(v),
            _ => None,
        }
    }

    /// 复数视图。
    pub fn as_complex(&self) -> Option<&Complex> {
        match self {
            Self::Complex(z) => Some(z),
            _ => None,
        }
    }

    /// 区间视图。
    pub fn as_interval(&self) -> Option<&Interval> {
        match self {
            Self::Interval(i) => Some(i),
            _ => None,
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

    /// Limb1/Limb2（及机器实数等）可栈拷贝时返回副本。
    pub fn clone_inline(&self) -> Option<Self> {
        match self {
            Self::Integer(n) => Some(Self::Integer(n.clone_inline()?)),
            Self::Rational(r) => Some(Self::Rational(r.clone_inline()?)),
            Self::Real(Real::Machine(x)) => Some(Self::Real(Real::Machine(*x))),
            _ => None,
        }
    }

    /// 可失败 owning 深复制（Living `19`）。
    pub fn try_clone_in(&self, ctx: &crate::policy::execution_budget::NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(match self {
            Self::Integer(n) => Self::Integer(n.try_clone_in(ctx)?),
            Self::Rational(r) => Self::Rational(r.try_clone_in(ctx)?),
            Self::Real(Real::Machine(x)) => Self::Real(Real::Machine(*x)),
            Self::Real(Real::Decimal(d)) => Self::Real(Real::Decimal(d.try_clone_in(ctx)?)),
            Self::Complex(z) => Self::Complex(z.try_clone_in(ctx)?),
            Self::Interval(i) => Self::Interval(i.try_clone_in(ctx)?),
            Self::Algebraic(a) => Self::Algebraic(a.try_clone_in(ctx)?),
            Self::Modular(m) => Self::Modular(m.try_clone_in(ctx)?),
            Self::FiniteField(v) => Self::FiniteField(v.try_clone_in(ctx)?),
            Self::PAdic(v) => Self::PAdic(v.try_clone_in(ctx)?),
        })
    }
}

#[cfg(test)]
mod fingerprint_content_tests {
    use super::*;
    use crate::integer::Integer;

    #[test]
    fn integer_content_hash_is_limb_stable() {
        let a = Number::Integer(Integer::from_i64(42));
        let b = Number::Integer(Integer::from_i64(42));
        let c = Number::Integer(Integer::from_i64(43));
        assert_eq!(a.fingerprint_content_hash(), b.fingerprint_content_hash());
        assert_ne!(a.fingerprint_content_hash(), c.fingerprint_content_hash());
        assert_eq!(a.fingerprint_domain_tag(), 1);
    }
}
