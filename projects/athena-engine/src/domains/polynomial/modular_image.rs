//! 多项式模同态：ℤ / ℚ → 𝔽_p 系数像（Living `30` G1 modular image）。

use athena_numeric::{Integer, Modulus, Number, Rational};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    builder::PolynomialBuilder,
    object::{CanonicalPolynomial, Polynomial},
    ring::CoefficientDomain,
    ring_table::RingTable,
};
use crate::{
    domains::number_theory::{
        CrtResult, NumberTheoryResult, NumberTheoryValue, RationalReconstruction, RationalReconstructionFailure, chinese_remainder,
        rational_reconstruction,
    },
    runtime::values::numeric_clone::clone_modulus,
};
use std::collections::BTreeSet;

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

impl ModularImage {
    /// Owning 复制（[`Modulus`] 无 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            source_ring: self.source_ring,
            image_ring: self.image_ring,
            image: self.image.owning_copy(),
            modulus: clone_modulus(&self.modulus),
            vanished: self.vanished,
        }
    }
}

impl Clone for ModularImage {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
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

/// 从模 `p` 剩余重构有理系数（默认 Wang 界）。
pub fn reconstruct_rational_coefficient(residue: &Number, modulus: &Modulus) -> Result<Number> {
    let integer = residue.as_integer().ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "reconstruct_requires_integer_residue")
    })?;
    match rational_reconstruction(integer, modulus, None, None) {
        RationalReconstruction::Found { value } => Ok(number_from_rational(value)),
        RationalReconstruction::NotFound { reason } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "rational_reconstruction_failed")
            .detail("reason", rational_reconstruction_failure_token(reason))),
    }
}

/// 将 𝔽_p 上多项式系数逐项有理重构到 ℚ（或 ℤ）目标环。
///
/// 目标环须为 [`CoefficientDomain::Rational`] 或 [`CoefficientDomain::Integer`]，
/// 且变量表 / 单项式序与像环一致。重构失败或整数目标遇到非整数分数时返回诊断。
pub fn reconstruct_polynomial_from_modular_image(
    image: &Polynomial,
    modulus: &Modulus,
    target_ring: RingId,
    rings: &RingTable,
) -> Result<CanonicalPolynomial> {
    let image_desc = rings.get(image.ring()).ok_or_else(|| ring_unknown(image.ring()))?;
    let target_desc = rings.get(target_ring).ok_or_else(|| ring_unknown(target_ring))?;
    let target_domain = rings.coefficient_domain_for_descriptor(target_desc).ok_or_else(|| ring_unknown(target_ring))?;
    match target_domain {
        CoefficientDomain::Integer | CoefficientDomain::Rational => {}
        _ => {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("operation", "reconstruct_target_must_be_z_or_q"));
        }
    }
    if image_desc.variables != target_desc.variables || image_desc.order != target_desc.order {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "reconstruct_ring_shape_mismatch"));
    }
    let require_integer = matches!(target_domain, CoefficientDomain::Integer);
    let mut builder = PolynomialBuilder::new(target_ring);
    for term in image.terms() {
        let mut coeff = reconstruct_rational_coefficient(term.coefficient(), modulus)?;
        if require_integer {
            match coeff.as_rational() {
                Some(r) if r.is_integer() => {
                    coeff = Number::integer(r.numerator());
                }
                Some(_) => {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("domain", "polynomial")
                        .detail("operation", "reconstruct_non_integer_for_z_target"));
                }
                None => {}
            }
        }
        if coeff.is_zero() {
            continue;
        }
        builder.push_term(coeff, term.exponents().to_vec())?;
    }
    builder.build(rings)
}

/// 从像环的素域 presentation 取出模数，再做有理重构。
pub fn reconstruct_polynomial_from_finite_field_ring(
    image: &Polynomial,
    target_ring: RingId,
    rings: &RingTable,
) -> Result<CanonicalPolynomial> {
    let modulus = modulus_of_finite_field_ring(image.ring(), rings)?;
    reconstruct_polynomial_from_modular_image(image, &modulus, target_ring, rings)
}

/// 多素数 CRT 合并结果（整数剩余类多项式，模为 `lcm`）。
#[derive(Debug, PartialEq)]
pub struct CrtPolynomialCombination {
    /// 合并后的模数（素数之积 / lcm）。
    pub modulus: Modulus,
    /// 在 ℤ 目标环上的规范多项式（系数 ∈ `[0, M)`）。
    pub polynomial: CanonicalPolynomial,
}

/// 将同一源多项式在多个素数下的像做 CRT 合并到 ℤ 环。
///
/// - 至少两个像；任一侧 `vanished` 仍可作为零像参与（缺项按 0 剩余）。
/// - 各像环须同变量 / 同序，且与 `integer_ring` 形状一致。
/// - `integer_ring` 须为 [`CoefficientDomain::Integer`]。
pub fn crt_combine_modular_images(
    images: &[ModularImage],
    integer_ring: RingId,
    rings: &RingTable,
) -> Result<CrtPolynomialCombination> {
    if images.len() < 2 {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_requires_at_least_two_images"));
    }
    let int_desc = rings.get(integer_ring).ok_or_else(|| ring_unknown(integer_ring))?;
    let int_domain = rings.coefficient_domain_for_descriptor(int_desc).ok_or_else(|| ring_unknown(integer_ring))?;
    if !matches!(int_domain, CoefficientDomain::Integer) {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_target_must_be_integer_ring"));
    }
    for img in images {
        let desc = rings.get(img.image_ring).ok_or_else(|| ring_unknown(img.image_ring))?;
        if desc.variables != int_desc.variables || desc.order != int_desc.order {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "crt_combine_ring_shape_mismatch"));
        }
    }

    let mut support: BTreeSet<Vec<u32>> = BTreeSet::new();
    for img in images {
        for term in img.image.terms() {
            support.insert(term.exponents().to_vec());
        }
    }

    let moduli: Vec<Modulus> = images.iter().map(|i| clone_modulus(&i.modulus)).collect();
    let mut builder = PolynomialBuilder::new(integer_ring);
    let mut combined_modulus: Option<Modulus> = None;
    for exponents in support {
        let mut residues = Vec::with_capacity(images.len());
        for img in images {
            let coeff = img
                .image
                .terms()
                .iter()
                .find(|t| t.exponents() == exponents.as_slice())
                .map(|t| t.coefficient())
                .and_then(Number::as_integer)
                .map(|n| clone_integer_from(n))
                .unwrap_or_else(Integer::zero);
            residues.push(coeff);
        }
        let (residue, modulus) = crt_residue_system(&residues, &moduli)?;
        if combined_modulus.is_none() {
            combined_modulus = Some(clone_modulus(&modulus));
        }
        if !residue.is_zero() {
            builder.push_term(Number::integer(residue), exponents)?;
        }
    }
    let modulus = combined_modulus.unwrap_or_else(|| clone_modulus(&moduli[0]));
    let polynomial = builder.build(rings)?;
    Ok(CrtPolynomialCombination { modulus, polynomial })
}

/// CRT 合并后再对系数做 Wang 有理重构到 ℚ / ℤ 目标环。
pub fn crt_combine_and_reconstruct(
    images: &[ModularImage],
    integer_ring: RingId,
    target_ring: RingId,
    rings: &RingTable,
) -> Result<CanonicalPolynomial> {
    let combined = crt_combine_modular_images(images, integer_ring, rings)?;
    reconstruct_polynomial_from_modular_image(&combined.polynomial, &combined.modulus, target_ring, rings)
}

fn crt_residue_system(residues: &[Integer], moduli: &[Modulus]) -> Result<(Integer, Modulus)> {
    match chinese_remainder(residues, moduli) {
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Crt(CrtResult::Consistent { solution, modulus_lcm }),
        } => Ok((solution.residue(), modulus_lcm)),
        NumberTheoryResult::Exact {
            value: NumberTheoryValue::Crt(CrtResult::Inconsistent { .. }),
        } => Err(Diagnostic::new(DiagnosticCode::CongruenceInconsistent)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_inconsistent")),
        NumberTheoryResult::Unevaluated { reason } | NumberTheoryResult::InvalidInput { reason } => Err(reason),
        NumberTheoryResult::Exact { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_unexpected_result")
            .detail("kind", "exact_non_crt")),
        NumberTheoryResult::Probable { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_unexpected_result")
            .detail("kind", "probable")),
        NumberTheoryResult::Partial { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_unexpected_result")
            .detail("kind", "partial")),
        NumberTheoryResult::ResourceLimited { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_unexpected_result")
            .detail("kind", "resource_limited")),
        NumberTheoryResult::Inconclusive { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "crt_combine_unexpected_result")
            .detail("kind", "inconclusive")),
    }
}

fn rational_reconstruction_failure_token(reason: RationalReconstructionFailure) -> &'static str {
    match reason {
        RationalReconstructionFailure::InvalidBounds => "invalid_bounds",
        RationalReconstructionFailure::NoCandidate => "no_candidate",
    }
}

fn modulus_of_finite_field_ring(ring: RingId, rings: &RingTable) -> Result<Modulus> {
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    let domain = rings.coefficient_domain_for_descriptor(desc).ok_or_else(|| ring_unknown(ring))?;
    let CoefficientDomain::FiniteField { field } = domain
    else {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "reconstruct_image_must_be_finite_field"));
    };
    rings.field_table().prime_modulus(*field)
}

fn clone_integer_from(n: &Integer) -> Integer {
    crate::runtime::values::numeric_clone::clone_integer(n)
}

fn number_from_rational(value: Rational) -> Number {
    if value.is_integer() {
        Number::integer(value.numerator())
    }
    else {
        Number::rational(value)
    }
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
