//! 多项式 canonical 构造（合并 · 排序 · 丢弃零项）。

use std::collections::HashMap;

use athena_numeric::{Number, add as coeff_add};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    coefficient_kernel::CoefficientRing,
    object::{CanonicalPolynomial, MonomialTerm, Polynomial},
    ring::{CoefficientDomain, RingDescriptor},
    ring_table::RingTable,
};
use crate::runtime::values::numeric_clone::clone_number;

/// 将任意项列表规范化为 [`CanonicalPolynomial`]（须已注册 [`RingId`]）。
pub fn canonicalize_polynomial(poly: Polynomial, rings: &RingTable) -> Result<CanonicalPolynomial> {
    let (ring, terms) = poly.into_parts();
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    canonicalize_terms(ring, desc, terms, rings)
}

pub(crate) fn canonicalize_terms(
    ring: RingId,
    desc: &RingDescriptor,
    raw: Vec<MonomialTerm>,
    rings: &RingTable,
) -> Result<CanonicalPolynomial> {
    let n = desc.variable_count();
    let coeff_ring = rings.coefficient_kernel(ring).ok();
    let mut acc: HashMap<Vec<u32>, Number> = HashMap::new();

    for term in raw {
        let (coefficient, exponents) = term.into_parts();
        if exponents.len() != n {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "canonicalize_exponent_length"));
        }
        validate_coefficient(&coefficient, rings.coefficient_domain_for_descriptor(desc).ok_or_else(|| ring_unknown(ring))?)?;
        if coefficient.is_zero() {
            continue;
        }
        match acc.get_mut(&exponents) {
            Some(existing) => {
                *existing = merge_coefficients(clone_number(existing), coefficient, coeff_ring.as_ref())?;
            }
            None => {
                acc.insert(exponents, coefficient);
            }
        }
    }

    let mut terms: Vec<MonomialTerm> = acc
        .into_iter()
        .filter(|(_, c)| !c.is_zero())
        .map(|(exponents, coefficient)| MonomialTerm::from_parts(coefficient, exponents))
        .collect();

    sort_terms_desc(&mut terms, &desc.monomial_layout)?;

    Ok(CanonicalPolynomial::from_canonical_parts(ring, terms))
}

fn sort_terms_desc(terms: &mut [MonomialTerm], layout: &super::monomial_layout::MonomialLayout) -> Result<()> {
    let mut sort_error = None;
    terms.sort_by(|a, b| {
        if sort_error.is_some() {
            return std::cmp::Ordering::Equal;
        }
        if let Err(d) = layout.validate_exponents(a.exponents()) {
            sort_error = Some(d);
            return std::cmp::Ordering::Equal;
        }
        if let Err(d) = layout.validate_exponents(b.exponents()) {
            sort_error = Some(d);
            return std::cmp::Ordering::Equal;
        }
        layout.cmp_exponents_desc(a.exponents(), b.exponents())
    });
    sort_error.map_or(Ok(()), Err)
}

fn merge_coefficients(a: Number, b: Number, coeff_ring: Option<&CoefficientRing<'_>>) -> Result<Number> {
    match coeff_ring {
        Some(ring) => ring.add(a, b),
        None => coeff_add(a, b),
    }
}

fn validate_coefficient(coeff: &Number, domain: &CoefficientDomain) -> Result<()> {
    match domain {
        CoefficientDomain::Integer => match coeff {
            Number::Integer(_) => Ok(()),
            Number::Rational(r) if r.is_integer() => Ok(()),
            _ => Err(coeff_mismatch("integer")),
        },
        CoefficientDomain::Rational => match coeff {
            Number::Integer(_) | Number::Rational(_) => Ok(()),
            _ => Err(coeff_mismatch("rational")),
        },
        CoefficientDomain::ModularInteger { .. } | CoefficientDomain::FiniteField { .. } => match coeff {
            Number::Integer(_) => Ok(()),
            _ => Err(coeff_mismatch("finite_field_skeleton")),
        },
        CoefficientDomain::ApproximateReal => Err(coeff_mismatch("approximate")),
    }
}

fn coeff_mismatch(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "polynomial").detail("operation", op)
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
