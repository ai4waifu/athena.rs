//! 统一数值值（Living `16`：私有字段 + 经验证构造）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    algebraic::AlgebraicNumber, complex::Complex, domain::NumericDomain, finite_field::FiniteFieldValue, integer::Integer,
    interval::Interval, modular::ModularValue, p_adic::PAdicValue, precision::PrecisionInfo, rational::Rational, real::Real,
};

/// 数值来源 / 证明引用占位。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NumericProvenance {
    /// 标签列表（骨架；后续换 ProofRef）。
    pub tags: Vec<String>,
}

/// 内部表示（由 [`NumericValue`] 构造器保证与 domain 一致）。
#[derive(Debug, Clone, PartialEq)]
pub enum NumericRepr {
    /// 整数。
    Integer(Integer),
    /// 有理。
    Rational(Rational),
    /// 实。
    Real(Real),
    /// 复。
    Complex(Complex),
    /// 区间。
    Interval(Interval),
    /// 代数。
    Algebraic(AlgebraicNumber),
    /// 模。
    Modular(ModularValue),
    /// 有限域。
    FiniteField(FiniteFieldValue),
    /// p-adic。
    PAdic(PAdicValue),
}

/// 带域与精度的数值（唯一执行真相源）。
#[derive(Debug, Clone, PartialEq)]
pub struct NumericValue {
    domain: NumericDomain,
    repr: NumericRepr,
    precision: PrecisionInfo,
    provenance: NumericProvenance,
}

/// Living `16` 过渡别名：公共面统一称 [`NumericValue`]。
pub type Number = NumericValue;

impl NumericValue {
    fn new_unchecked(
        domain: NumericDomain,
        repr: NumericRepr,
        precision: PrecisionInfo,
        provenance: NumericProvenance,
    ) -> Self {
        Self { domain, repr, precision, provenance }
    }

    /// 经验证构造；domain 与 repr 不一致时失败。
    pub fn try_new(
        domain: NumericDomain,
        repr: NumericRepr,
        precision: PrecisionInfo,
        provenance: NumericProvenance,
    ) -> Result<Self> {
        let v = Self::new_unchecked(domain, repr, precision, provenance);
        v.validate()?;
        Ok(v)
    }

    /// 校验 domain / repr / precision 不变量。
    pub fn validate(&self) -> Result<()> {
        let ok = match (&self.domain, &self.repr) {
            (NumericDomain::Integer, NumericRepr::Integer(_)) => true,
            (NumericDomain::Rational, NumericRepr::Rational(_)) => true,
            (NumericDomain::Real, NumericRepr::Real(Real::Machine(_))) => {
                self.precision.kind == crate::precision::PrecisionKind::Machine
            }
            (NumericDomain::Real, NumericRepr::Real(Real::Unsupported)) => false,
            (NumericDomain::Complex, NumericRepr::Complex(_)) => true,
            // 未开放稳定构造的域：禁止经 try_new 进入（骨架类型仍可内部保留）
            _ => false,
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
        Self::new_unchecked(
            NumericDomain::Integer,
            NumericRepr::Integer(n),
            PrecisionInfo::exact(),
            NumericProvenance::default(),
        )
    }

    /// 精确有理（既约；分母为 1 时仍保留 Rational 域，除非调用方用 [`Self::from_rational_normalized`]）。
    pub fn rational(r: Rational) -> Self {
        Self::new_unchecked(
            NumericDomain::Rational,
            NumericRepr::Rational(r),
            PrecisionInfo::exact(),
            NumericProvenance::default(),
        )
    }

    /// 有理规范化：分母为 1 时降为 Integer 域。
    pub fn from_rational_normalized(r: Rational) -> Self {
        if r.is_integer() { Self::integer(r.numerator()) } else { Self::rational(r) }
    }

    /// 机器实数。
    pub fn machine_real(x: f64) -> Self {
        Self::new_unchecked(
            NumericDomain::Real,
            NumericRepr::Real(Real::machine(x)),
            PrecisionInfo::machine(),
            NumericProvenance::default(),
        )
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

    /// 域。
    pub fn domain(&self) -> &NumericDomain {
        &self.domain
    }

    /// 表示。
    pub fn repr(&self) -> &NumericRepr {
        &self.repr
    }

    /// 精度。
    pub fn precision(&self) -> &PrecisionInfo {
        &self.precision
    }

    /// 来源。
    pub fn provenance(&self) -> &NumericProvenance {
        &self.provenance
    }

    /// 是否精确零。
    pub fn is_zero(&self) -> bool {
        match &self.repr {
            NumericRepr::Integer(n) => n.is_zero(),
            NumericRepr::Rational(r) => r.is_zero(),
            NumericRepr::Real(Real::Machine(x)) => *x == 0.0,
            _ => false,
        }
    }

    /// 是否精确一。
    pub fn is_one(&self) -> bool {
        match &self.repr {
            NumericRepr::Integer(n) => n.is_one(),
            NumericRepr::Rational(r) => r.is_integer() && r.numerator().is_one(),
            NumericRepr::Real(Real::Machine(x)) => *x == 1.0,
            _ => false,
        }
    }

    /// 是否精确 `-1`。
    pub fn is_neg_one(&self) -> bool {
        match &self.repr {
            NumericRepr::Integer(n) => n.to_i64() == Some(-1),
            NumericRepr::Rational(r) => r.is_integer() && r.numerator().to_i64() == Some(-1),
            NumericRepr::Real(Real::Machine(x)) => *x == -1.0,
            _ => false,
        }
    }

    /// 逻辑真值（精确非零 → true；NaN → false）。
    pub fn is_truthy(&self) -> bool {
        match &self.repr {
            NumericRepr::Integer(n) => !n.is_zero(),
            NumericRepr::Rational(r) => !r.is_zero(),
            NumericRepr::Real(Real::Machine(x)) => *x != 0.0 && !x.is_nan(),
            _ => false,
        }
    }

    /// 可落入 `i64` 的整数指数。
    pub fn as_integer_exp(&self) -> Option<i64> {
        match &self.repr {
            NumericRepr::Integer(n) => n.to_i64(),
            NumericRepr::Rational(r) if r.is_integer() => r.numerator().to_i64(),
            _ => None,
        }
    }

    /// 同 [`Self::as_integer_exp`]。
    pub fn as_exact_integer(&self) -> Option<i64> {
        self.as_integer_exp()
    }

    /// 机器 `f64`（仅 Machine Real）。
    pub fn as_machine_f64(&self) -> Option<f64> {
        match &self.repr {
            NumericRepr::Real(Real::Machine(x)) => Some(*x),
            _ => None,
        }
    }

    /// 渲染字符串。
    pub fn to_render_string(&self) -> String {
        match &self.repr {
            NumericRepr::Integer(n) => n.to_decimal_string(),
            NumericRepr::Rational(r) => r.to_wire_string(),
            NumericRepr::Real(Real::Machine(x)) => {
                if x.fract() == 0.0 && x.abs() < 1e15 {
                    format!("{}", *x as i64)
                }
                else {
                    format!("{x}")
                }
            }
            _ => format!("{:?}", self.repr),
        }
    }

    /// 整数视图。
    pub fn as_integer(&self) -> Option<&Integer> {
        match &self.repr {
            NumericRepr::Integer(n) => Some(n),
            _ => None,
        }
    }

    /// 有理视图。
    pub fn as_rational(&self) -> Option<&Rational> {
        match &self.repr {
            NumericRepr::Rational(r) => Some(r),
            _ => None,
        }
    }

    /// 实数视图。
    pub fn as_real(&self) -> Option<&Real> {
        match &self.repr {
            NumericRepr::Real(r) => Some(r),
            _ => None,
        }
    }
}
