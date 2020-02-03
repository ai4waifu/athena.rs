//! 多项式模同态：ℤ / ℚ → 𝔽_p 系数像（Living `30` G1 modular image）。

use athena_numeric::{Integer, Modulus, Number, Rational};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    builder::PolynomialBuilder,
    object::{CanonicalPolynomial, Polynomial},
    ring::CoefficientDomain,
    ring_table::RingTable,
};

/// 单次模同态结果（候选像，不进 M-Graph）。
#[derive(Debug, PartialEq)]
pub struct ModularImage {
    /// 源环。
    pub source_ring: RingId,
    /// 像环（𝔽_p 上同变量 / 同序）。
    pub image_ring: RingId,
    /// 模约化后的规范多项式。
    pub image: CanonicalPolynomial,
    /// 使用的素数模数。
    pub modulus: Modulus,
    /// 源非零但像为零（坏素数 / 整体系数全被约掉）。
    pub vanished: bool,
}

/// 将 ℤ 或 ℚ 系数多项式映射到已注册的 𝔽_p 多项式环。
///
/// - 源环须为 [`CoefficientDomain::Integer`] 或 [`CoefficientDomain::Rational`]。
/// - 像环须为 [`CoefficientDomain::FiniteField`]，且变量表与单项式序与源环一致。
/// - 有理系数 `n/d` 要求 `d ≢ 0 (mod p)`，否则返回诊断（坏素数）。
pub fn map_polynomial_mod_prime(poly: &Polynomial, image_ring: RingId, rings: &RingTable) -> Result<ModularImage> {
    let source_ring = poly.ring();
    let source_desc = rings.get(source_ring).ok_or_else(|| ring_unknown(source_ring))?;
    let image_desc = rings.get(image_ring).ok_or_else(|| ring_unknown(image_ring))?;
    let source_domain = rings.coefficient_domain_for_descriptor(source_desc).ok_or_else(|| ring_unknown(source_ring))?;
    let image_domain = rings.coefficient_domain_for_descriptor(image_desc).ok_or_else(|| ring_unknown(image_ring))?;

    match source_domain {
        CoefficientDomain::Integer | CoefficientDomain::Rational => {}
        _ => {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("operation", "modular_image_source_must_be_z_or_q"));
        }
    }
    let CoefficientDomain::FiniteField { field } = image_domain
    else {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "modular_image_target_must_be_finite_field"));
    };
    if source_desc.variables != image_desc.variables || source_desc.order != image_desc.order {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "modular_image_ring_shape_mismatch"));
    }

    let modulus = rings.field_table().prime_modulus(*field)?;
    let mut builder = PolynomialBuilder::new(image_ring);
    for term in poly.terms() {
        let reduced = reduce_coefficient(term.coefficient(), &modulus)?;
        if reduced.is_zero() {
            continue;
        }
        builder.push_term(reduced, term.exponents().to_vec())?;
    }
    let image = builder.build(rings)?;
    let vanished = !poly.is_zero() && image.is_zero();
    Ok(ModularImage { source_ring, image_ring, image, modulus, vanished })
}

/// 批量映射生成元；任一坏分母则整体失败。
pub fn map_generators_mod_prime(generators: &[Polynomial], image_ring: RingId, rings: &RingTable) -> Result<Vec<ModularImage>> {
    generators.iter().map(|g| map_polynomial_mod_prime(g, image_ring, rings)).collect()
}

fn reduce_coefficient(coeff: &Number, modulus: &Modulus) -> Result<Number> {
    if let Some(n) = coeff.as_integer() {
        return Ok(Number::integer(modulus.reduce(n)));
    }
    if let Some(r) = coeff.as_rational() {
        return reduce_rational(r, modulus);
    }
    // Builder 可能把整数有理写成 Rational；也接受可整化的有理。
    Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
        .detail("domain", "polynomial")
        .detail("operation", "modular_image_coeff_must_be_integer_or_rational"))
}

fn reduce_rational(r: &Rational, modulus: &Modulus) -> Result<Number> {
    let numer = r.numerator();
    let denom = r.denominator();
    let n = modulus.reduce(&numer);
    let d = modulus.reduce(&denom);
    if d.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing)
            .detail("domain", "polynomial")
            .detail("operation", "modular_image_bad_denominator")
            .detail("modulus", modulus.value().to_decimal_string()));
    }
    if d.is_one() {
        return Ok(Number::integer(n));
    }
    // n * d^{-1} mod p via Fp kernel path on Integer residues.
    let inv = crate::domains::number_theory::mod_inverse(&d, modulus)?;
    let inv_res = inv.residue();
    let product = n.mul(&inv_res);
    Ok(Number::integer(modulus.reduce(&product)))
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
