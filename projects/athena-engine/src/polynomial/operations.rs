//! 多项式精确加减乘（ℤ / ℚ / 𝔽_p 系数域）。

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    canonical::canonicalize_terms,
    exponent::add_exponent_vectors,
    expr::{MonomialTerm, Polynomial},
    ring_table::RingTable,
};
use crate::numeric_clone::clone_number;

/// 同环多项式加法。
pub fn add_polynomial(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    if lhs.is_zero() {
        return Ok(rhs);
    }
    if rhs.is_zero() {
        return Ok(lhs);
    }
    let ring = lhs.ring();
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    rings.coeff_kernel(ring)?;
    let mut raw = lhs.into_parts().1;
    raw.extend(rhs.into_parts().1);
    canonicalize_terms(ring, desc, raw, rings)
}

/// 同环多项式减法。
pub fn sub_polynomial(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    if rhs.is_zero() {
        return Ok(lhs);
    }
    let ring = lhs.ring();
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    let coeff = rings.coeff_kernel(ring)?;
    validate_exponent_lengths(&lhs, desc.variable_count())?;
    validate_exponent_lengths(&rhs, desc.variable_count())?;
    let mut raw = lhs.into_parts().1;
    for term in rhs.into_parts().1 {
        let (c, exponents) = term.into_parts();
        raw.push(MonomialTerm::from_parts(coeff.neg(c)?, exponents));
    }
    canonicalize_terms(ring, desc, raw, rings)
}

/// 同环多项式乘法。
pub fn mul_polynomial(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    if lhs.is_zero() || rhs.is_zero() {
        return Ok(Polynomial::zero(lhs.ring()));
    }
    let ring = lhs.ring();
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    let coeff = rings.coeff_kernel(ring)?;
    let n = desc.variable_count();
    validate_exponent_lengths(&lhs, n)?;
    validate_exponent_lengths(&rhs, n)?;
    let mut raw = Vec::new();
    for lt in lhs.terms() {
        for rt in rhs.terms() {
            let exponents = add_exponent_vectors(lt.exponents(), rt.exponents())?;
            raw.push(MonomialTerm::from_parts(
                coeff.mul(clone_number(lt.coefficient()), clone_number(rt.coefficient()))?,
                exponents,
            ));
        }
    }
    canonicalize_terms(ring, desc, raw, rings)
}

fn validate_exponent_lengths(poly: &Polynomial, n: usize) -> Result<()> {
    for term in poly.terms() {
        if term.exponents().len() != n {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "exponent_length"));
        }
    }
    Ok(())
}

fn ensure_same_ring(lhs: &Polynomial, rhs: &Polynomial) -> Result<()> {
    if lhs.ring() != rhs.ring() {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "ring_mismatch"));
    }
    Ok(())
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
