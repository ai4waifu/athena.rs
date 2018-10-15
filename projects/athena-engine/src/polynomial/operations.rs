//! 多项式精确加减乘（ℤ / ℚ / 𝔽_p 系数域）。

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    canonical::canonicalize_terms,
    coeff_kernel::CoeffRing,
    expr::{MonomialTerm, Polynomial},
    ring_table::RingTable,
};

/// 同环多项式加法。
pub fn add_polynomial(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    if lhs.terms.is_empty() {
        return Ok(rhs);
    }
    if rhs.terms.is_empty() {
        return Ok(lhs);
    }
    let desc = rings.get(lhs.ring).ok_or_else(|| ring_unknown(lhs.ring))?;
    CoeffRing::new(&desc.coefficients)?;
    let mut raw = lhs.terms;
    raw.extend(rhs.terms);
    canonicalize_terms(lhs.ring, desc, raw)
}

/// 同环多项式减法。
pub fn sub_polynomial(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    if rhs.terms.is_empty() {
        return Ok(lhs);
    }
    let desc = rings.get(lhs.ring).ok_or_else(|| ring_unknown(lhs.ring))?;
    let coeff = CoeffRing::new(&desc.coefficients)?;
    validate_exponent_lengths(&lhs, desc.variable_count())?;
    validate_exponent_lengths(&rhs, desc.variable_count())?;
    let mut raw = lhs.terms;
    for term in rhs.terms {
        raw.push(MonomialTerm {
            coefficient: coeff.neg(term.coefficient)?,
            exponents: term.exponents,
        });
    }
    canonicalize_terms(lhs.ring, desc, raw)
}

/// 同环多项式乘法。
pub fn mul_polynomial(lhs: Polynomial, rhs: Polynomial, rings: &RingTable) -> Result<Polynomial> {
    ensure_same_ring(&lhs, &rhs)?;
    if lhs.terms.is_empty() || rhs.terms.is_empty() {
        return Ok(Polynomial::zero(lhs.ring));
    }
    let desc = rings.get(lhs.ring).ok_or_else(|| ring_unknown(lhs.ring))?;
    let coeff = CoeffRing::new(&desc.coefficients)?;
    let n = desc.variable_count();
    validate_exponent_lengths(&lhs, n)?;
    validate_exponent_lengths(&rhs, n)?;
    let mut raw = Vec::new();
    for lt in &lhs.terms {
        for rt in &rhs.terms {
            let exponents: Vec<u32> = lt
                .exponents
                .iter()
                .zip(&rt.exponents)
                .map(|(a, b)| a.saturating_add(*b))
                .collect();
            raw.push(MonomialTerm {
                coefficient: coeff.mul(lt.coefficient.clone(), rt.coefficient.clone())?,
                exponents,
            });
        }
    }
    canonicalize_terms(lhs.ring, desc, raw)
}

fn validate_exponent_lengths(poly: &Polynomial, n: usize) -> Result<()> {
    for term in &poly.terms {
        if term.exponents.len() != n {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "exponent_length"));
        }
    }
    Ok(())
}

fn ensure_same_ring(lhs: &Polynomial, rhs: &Polynomial) -> Result<()> {
    if lhs.ring != rhs.ring {
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
