//! 𝔽_{p^n} 系数专用内核（多项式基坐标）。

use athena_numeric::{FiniteFieldValue, Integer, Modulus, Number, NumericValue};
use athena_types::{Diagnostic, DiagnosticCode, FieldId, Result};

use crate::domains::algebra::{FiniteFieldPolySpec, add_coords, canonical_coords, inv_coords, mul_coords};

/// 扩张有限域系数算术。
#[derive(Debug)]
pub struct FpExtCoeffKernel {
    field: FieldId,
    spec: FiniteFieldPolySpec,
    modulus: Modulus,
}

impl FpExtCoeffKernel {
    /// 由已注册 𝔽_{p^n} presentation 构造。
    pub fn new(field: FieldId, spec: FiniteFieldPolySpec, modulus: Modulus) -> Self {
        Self { field, spec, modulus }
    }

    /// 域句柄。
    pub fn field(&self) -> FieldId {
        self.field
    }

    /// 系数加法。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        let x = self.extract_coords(&a)?;
        let y = self.extract_coords(&b)?;
        Ok(self.pack_coords(add_coords(&x, &y, &self.modulus)))
    }

    /// 系数乘法。
    pub fn mul(&self, a: Number, b: Number) -> Result<Number> {
        let x = self.extract_coords(&a)?;
        let y = self.extract_coords(&b)?;
        Ok(self.pack_coords(mul_coords(&x, &y, &self.spec, &self.modulus)))
    }

    /// 系数取负。
    pub fn neg(&self, a: Number) -> Result<Number> {
        let x = self.extract_coords(&a)?;
        let p = self.modulus.value();
        let neg = x.iter().map(|c| self.modulus.reduce(&p.sub(c))).collect();
        Ok(self.pack_coords(neg))
    }

    /// 扩张域是域。
    pub fn is_field(&self) -> bool {
        true
    }

    /// 域除法。
    pub fn div(&self, a: Number, b: Number) -> Result<Number> {
        let y = self.extract_coords(&b)?;
        if y.iter().all(|c| c.is_zero()) {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "polynomial"));
        }
        let inv = inv_coords(&y, &self.spec, &self.modulus)?;
        let x = self.extract_coords(&a)?;
        Ok(self.pack_coords(mul_coords(&x, &inv, &self.spec, &self.modulus)))
    }

    /// 乘法逆元。
    pub fn inv(&self, a: Number) -> Result<Number> {
        self.div(Number::integer(Integer::one()), a)
    }

fn extract_coords(&self, coeff: &Number) -> Result<Vec<Integer>> {
        match coeff {
            NumericValue::FiniteField(ff) if ff.field() == self.field => {
                canonical_coords(ff.coefficients().to_vec(), self.spec.degree, &self.modulus)
            }
            NumericValue::Integer(n) => canonical_coords(vec![self.modulus.reduce(n)], self.spec.degree, &self.modulus),
            _ => Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "fp_ext_coeff_required")),
        }
    }

    fn pack_coords(&self, coords: Vec<Integer>) -> Number {
        Number::FiniteField(
            FiniteFieldValue::try_new(self.field, athena_types::FieldPresentationId(0), coords)
                .expect("coords non-empty from kernel"),
        )
    }
}
