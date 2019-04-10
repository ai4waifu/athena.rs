//! ℤ 系数专用内核。

use athena_numeric::{Number, add as num_add, mul as num_mul, neg as num_neg};
use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 整数环系数算术。
#[derive(Debug, Copy, Clone, Default)]
pub struct ZCoeffKernel;

impl ZCoeffKernel {
    /// 系数加法。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        num_add(a, b)
    }

    /// 系数乘法。
    pub fn mul(&self, a: Number, b: Number) -> Result<Number> {
        num_mul(a, b)
    }

    /// 系数取负。
    pub fn neg(&self, a: Number) -> Result<Number> {
        Ok(num_neg(a))
    }

    /// ℤ 不是域。
    pub fn is_field(&self) -> bool {
        false
    }

    /// 域除法（ℤ 上禁止）。
    pub fn div(&self, _a: Number, _b: Number) -> Result<Number> {
        Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "integer_field_division"))
    }

    /// 乘法逆元（ℤ 上禁止）。
    pub fn inv(&self, _a: Number) -> Result<Number> {
        Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "integer_field_division"))
    }
}
