//! Canonical polynomial construction（merge · sort · drop zero）。

use std::collections::HashMap;

use athena_numeric::{Number, add as coeff_add};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    coeff_kernel::CoeffRing,
    expr::{MonomialTerm, Polynomial},
    ring::{CoefficientDomain, RingDescriptor},
    ring_table::RingTable,
};

/// 将任意项列表规范化为 [`Polynomial`]（须已注册 [`RingId`]）。
pub fn canonicalize_polynomial(poly: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    let desc = rings.get(poly.ring).ok_or_else(|| ring_unknown(poly.ring))?;
    canonicalize_terms(poly.ring, desc, poly.terms)
}

pub(crate) fn canonicalize_terms(ring: RingId, desc: &RingDescriptor, raw: Vec<MonomialTerm>) -> Result<Polynomial> {
    let n = desc.variable_count();
    let coeff_ring = CoeffRing::new(&desc.coefficients).ok();
    let mut acc: HashMap<Vec<u32>, Number> = HashMap::new();

    for term in raw {
        if term.exponents.len() != n {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "canonicalize_exponent_length"));
        }
        validate_coefficient(&term.coefficient, &desc.coefficients)?;
        if term.coefficient.is_zero() {
            continue;
        }
        match acc.get_mut(&term.exponents) {
            Some(existing) => {
                *existing = merge_coefficients(existing.clone(), term.coefficient, coeff_ring.as_ref())?;
            }
            None => {
                acc.insert(term.exponents, term.coefficient);
            }
        }
    }

    let mut terms: Vec<MonomialTerm> = acc
        .into_iter()
        .filter(|(_, c)| !c.is_zero())
        .map(|(exponents, coefficient)| MonomialTerm { coefficient, exponents })
        .collect();

    terms.sort_by(|a, b| {
        desc.order
            .cmp_exponents(&b.exponents, &a.exponents, n)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Polynomial { ring, terms })
}

fn merge_coefficients(a: Number, b: Number, coeff_ring: Option<&CoeffRing<'_>>) -> Result<Number> {
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
        CoefficientDomain::PrimeField { .. }
        | CoefficientDomain::ModularInteger { .. }
        | CoefficientDomain::FiniteField { .. } => match coeff {
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
