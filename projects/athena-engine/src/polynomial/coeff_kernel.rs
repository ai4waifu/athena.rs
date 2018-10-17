//! 系数域精确算术（ℤ · ℚ · 𝔽_p）。

use athena_numeric::{Integer, Modulus, Number, NumericValue, add as num_add, mul as num_mul, neg as num_neg};
use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::ring::CoefficientDomain;

/// 绑定多项式系数域的系数环运算。
pub struct CoeffRing<'a> {
    domain: &'a CoefficientDomain,
    prime_modulus: Option<Modulus>,
}

impl<'a> CoeffRing<'a> {
    /// 构造系数环（𝔽_p 预建 [`Modulus`]）。
    pub fn new(domain: &'a CoefficientDomain) -> Result<Self> {
        if !domain.is_f3_supported() {
            return Err(unsupported_domain());
        }
        let prime_modulus = match domain {
            CoefficientDomain::PrimeField { p } => Some(Modulus::new(p.clone())?),
            _ => None,
        };
        Ok(Self { domain, prime_modulus })
    }

    /// 系数加法（合并同类项与 [`super::canonical`] 共用）。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        self.reduce(num_add(a, b)?)
    }

    /// 系数减法。
    #[allow(dead_code)]
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

    /// 系数域是否为域（Gröbner / 域除所需）。
    pub fn is_field(&self) -> bool {
        matches!(
            self.domain,
            CoefficientDomain::Rational | CoefficientDomain::PrimeField { .. }
        )
    }

    /// 域除法 `a / b`（`b` 须可逆）。
    pub fn div(&self, a: Number, b: Number) -> Result<Number> {
        if b.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "polynomial"));
        }
        match self.domain {
            CoefficientDomain::Rational => athena_numeric::div(a, b),
            CoefficientDomain::PrimeField { .. } => {
                let modulus = self.prime_modulus.as_ref().expect("prime modulus");
                let bi = extract_integer(&b)?;
                let inv = crate::number_theory::mod_inverse(&bi, modulus)?;
                self.mul(a, Number::integer(inv.residue().clone()))
            }
            CoefficientDomain::Integer => Err(field_required()),
            _ => Err(unsupported_domain()),
        }
    }

    /// 乘法逆元（域上）。
    pub fn inv(&self, a: Number) -> Result<Number> {
        self.div(Number::small_int(1), a)
    }

    fn reduce(&self, coeff: Number) -> Result<Number> {
        match self.domain {
            CoefficientDomain::PrimeField { .. } => {
                let modulus = self.prime_modulus.as_ref().expect("prime modulus");
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
            CoefficientDomain::Integer | CoefficientDomain::Rational | CoefficientDomain::PrimeField { .. }
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
