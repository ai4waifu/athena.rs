//! 数值域上下文（显式，禁止静默跨域）。

use athena_types::FieldId;

use crate::{integer::Integer, modular::Modulus};

/// 运算所处的数值域。
///
/// Living `19`：含 `Modulus` / `Integer` 时不 derive [`Clone`]。
#[derive(Debug, PartialEq, Eq)]
pub enum NumericDomain {
    /// ℤ。
    Integer,
    /// ℚ。
    Rational,
    /// ℝ（机器或任意精度由 [`crate::representation::precision::PrecisionInfo`] 区分）。
    Real,
    /// ℂ。
    Complex,
    /// 区间。
    Interval,
    /// 代数数。
    Algebraic,
    /// ℤ/nℤ。
    Modular {
        /// 模数。
        modulus: Modulus,
    },
    /// 有限域。
    FiniteField {
        /// 域 id。
        field: FieldId,
    },
    /// p-adic。
    PAdic {
        /// 素数 p（内部整数包装）。
        prime: Integer,
        /// 精度。
        precision: u32,
    },
}
