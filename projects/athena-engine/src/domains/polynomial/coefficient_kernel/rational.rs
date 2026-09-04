//! ℚ 系数专用内核。

use athena_numeric::{Number, add as num_add, div as num_div, mul as num_mul, neg as num_neg};
use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 有理数域系数算术。
#[derive(Debug, Copy, Clone, Default)]
pub struct QCoefficientKernel;

impl QCoefficientKernel {
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

    /// ℚ 是域。
    pub fn is_field(&self) -> bool {
        true
    }

    /// 域除法。
    pub fn div(&self, a: Number, b: Number) -> Result<Number> {
        if b.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "polynomial"));
        }
        num_div(a, b)
    }

    /// 乘法逆元。
    pub fn inv(&self, a: Number) -> Result<Number> {
        self.div(Number::small_int(1), a)
    }
}
