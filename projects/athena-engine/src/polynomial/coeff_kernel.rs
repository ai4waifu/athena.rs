//! 系数域精确算术（ℤ · ℚ · 𝔽_p / 经 FieldId 注册的素域）。

use athena_numeric::{Integer, Modulus, Number, NumericValue, add as num_add, mul as num_mul, neg as num_neg};
use athena_types::{CoefficientRingId, Diagnostic, DiagnosticCode, Result};

use super::{coeff_ring_table::CoeffRingTable, ring::CoefficientDomain};

/// 绑定多项式系数域的系数环运算。
pub struct CoeffRing<'a> {
    domain: &'a CoefficientDomain,
    prime_modulus: Option<&'a Modulus>,
}

impl<'a> CoeffRing<'a> {
    /// 由 [`CoefficientRingId`] 解析（算法入口选一次）。
    pub fn for_descriptor(coefficient_ring: CoefficientRingId, table: &'a CoeffRingTable) -> Result<Self> {
        let entry = table.entry(coefficient_ring)?;
        let domain = entry.domain();
        if !domain.is_f3_supported() {
            return Err(unsupported_domain());
        }
        Ok(Self { domain, prime_modulus: entry.prime_modulus() })
    }

    /// 构造系数环（legacy；优先 [`Self::for_descriptor`]）。
    pub fn new(domain: &'a CoefficientDomain) -> Result<Self> {
        if !domain.is_f3_supported() {
            return Err(unsupported_domain());
        }
        Ok(Self { domain, prime_modulus: None })
    }

    /// 系数加法。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        self.reduce(num_add(a, b)?)
    }

    /// 系数减法。
    pub fn sub(&self, a: Number, b: Number) -> Result<Number> {
        self.add(a, num_neg(b))
    }

    /// 系数乘法。
    pub fn mul(&self, a: Number, b: Number) -> Result<Number> {
        self.reduce(num_mul(a, b)?)
    }

    /// 系数取负。
    pub fn neg(&self, a: Number) -> Result<Number> {
        self.reduce(num_neg(a))
    }

    /// 系数域是否为域。
    pub fn is_field(&self) -> bool {
        matches!(
            self.domain,
            CoefficientDomain::Rational | CoefficientDomain::PrimeField { .. } | CoefficientDomain::FiniteField { .. }
        )
    }

    /// 域除法 `a / b`。
    pub fn div(&self, a: Number, b: Number) -> Result<Number> {
        if b.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "polynomial"));
        }
        match self.domain {
            CoefficientDomain::Rational => athena_numeric::div(a, b),
            CoefficientDomain::PrimeField { .. } | CoefficientDomain::FiniteField { .. } => {
                let modulus = self.modulus()?;
                let bi = extract_integer(&b)?;
                let inv = crate::number_theory::mod_inverse(&bi, &modulus)?;
                self.mul(a, Number::integer(inv.residue().clone()))
            }
            CoefficientDomain::Integer => Err(field_required()),
            _ => Err(unsupported_domain()),
        }
    }

    /// 乘法逆元。
    pub fn inv(&self, a: Number) -> Result<Number> {
        self.div(Number::small_int(1), a)
    }

    fn modulus(&self) -> Result<Modulus> {
        if let Some(m) = self.prime_modulus {
            return Ok(m.clone());
        }
        match self.domain {
            CoefficientDomain::PrimeField { p } => Modulus::new(p.clone()),
            CoefficientDomain::FiniteField { characteristic, .. } => Modulus::new(characteristic.clone()),
            _ => Err(unsupported_domain()),
        }
    }

    fn reduce(&self, coeff: Number) -> Result<Number> {
        match self.domain {
            CoefficientDomain::PrimeField { .. } | CoefficientDomain::FiniteField { .. } => {
                let modulus = self.modulus()?;
                let integer = extract_integer(&coeff)?;
                Ok(Number::integer(modulus.reduce(&integer)))
            }
            CoefficientDomain::Integer | CoefficientDomain::Rational => Ok(coeff),
            _ => Err(unsupported_domain()),
        }
    }
}

impl CoefficientDomain {
    /// F3 精确内核支持的系数域。
    pub fn is_f3_supported(&self) -> bool {
        matches!(
            self,
            CoefficientDomain::Integer
                | CoefficientDomain::Rational
                | CoefficientDomain::PrimeField { .. }
                | CoefficientDomain::FiniteField { .. }
        )
    }
}

fn extract_integer(coeff: &Number) -> Result<Integer> {
    match coeff {
        NumericValue::Integer(i) => Ok(i.clone()),
        _ => Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "coeff_integer_required")),
    }
}

fn unsupported_domain() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "coeff_domain_unsupported")
}

fn field_required() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
        .detail("domain", "polynomial")
        .detail("operation", "groebner_requires_field")
}
