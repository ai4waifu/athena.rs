//! ℤ/nℤ 系数专用内核（精确、一般非域）。

use athena_numeric::{Integer, Modulus, Number, NumericValue};
use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 模整数环系数算术。
#[derive(Debug, PartialEq, Eq)]
pub struct ZnCoefficientKernel {
    modulus: Modulus,
}

impl ZnCoefficientKernel {
    /// 由已验证模数构造。
    pub fn new(modulus: Modulus) -> Self {
        Self { modulus }
    }

    /// 模数。
    pub fn modulus(&self) -> &Modulus {
        &self.modulus
    }

    /// 系数加法。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        let x = self.reduce_number(&a)?;
        let y = self.reduce_number(&b)?;
        Ok(Number::integer(self.modulus.reduce(&x.add(&y))))
    }

    /// 系数乘法。
    pub fn mul(&self, a: Number, b: Number) -> Result<Number> {
        let x = self.reduce_number(&a)?;
        let y = self.reduce_number(&b)?;
        Ok(Number::integer(self.modulus.reduce(&x.mul(&y))))
    }

    /// 系数取负。
    pub fn neg(&self, a: Number) -> Result<Number> {
        let x = self.reduce_number(&a)?;
        Ok(Number::integer(self.modulus.reduce(&x.neg())))
    }

    /// ℤ/nℤ 一般不是域。
    pub fn is_field(&self) -> bool {
        false
    }

    /// 域除法（非域禁止）。
    pub fn div(&self, _a: Number, _b: Number) -> Result<Number> {
        Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "modular_integer_field_division"))
    }

    /// 乘法逆元（非域禁止）。
    pub fn inv(&self, _a: Number) -> Result<Number> {
        Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "modular_integer_field_division"))
    }

    fn reduce_number(&self, coeff: &Number) -> Result<Integer> {
        match coeff {
            NumericValue::Integer(n) => Ok(self.modulus.reduce(n)),
            NumericValue::Modular(m) => {
                if let Some(embedded) = m.modulus() {
                    if embedded.value() != self.modulus.value() {
                        return Err(coeff_mismatch());
                    }
                }
                Ok(self.modulus.reduce(m.residue()))
            }
            _ => Err(coeff_mismatch()),
        }
    }
}

fn coeff_mismatch() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
        .detail("domain", "polynomial")
        .detail("operation", "zn_coeff_integer_required")
}
