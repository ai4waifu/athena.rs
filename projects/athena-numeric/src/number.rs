//! 统一数值值。

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

/// 内部表示。
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

/// 带域与精度的数值。
#[derive(Debug, Clone, PartialEq)]
pub struct NumericValue {
    /// 域。
    pub domain: NumericDomain,
    /// 表示。
    pub value: NumericRepr,
    /// 精度。
    pub precision: PrecisionInfo,
    /// 来源。
    pub provenance: NumericProvenance,
}

impl NumericValue {
    /// 精确整数。
    pub fn integer(n: Integer) -> Self {
        Self {
            domain: NumericDomain::Integer,
            value: NumericRepr::Integer(n),
            precision: PrecisionInfo::exact(),
            provenance: NumericProvenance::default(),
        }
    }

    /// 精确有理。
    pub fn rational(r: Rational) -> Self {
        Self {
            domain: NumericDomain::Rational,
            value: NumericRepr::Rational(r),
            precision: PrecisionInfo::exact(),
            provenance: NumericProvenance::default(),
        }
    }

    /// 机器实数。
    pub fn machine_real(x: f64) -> Self {
        Self {
            domain: NumericDomain::Real,
            value: NumericRepr::Real(Real::machine(x)),
            precision: PrecisionInfo::machine(),
            provenance: NumericProvenance::default(),
        }
    }
}
