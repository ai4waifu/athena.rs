//! Gröbner 基（Buchberger）· 理想约化 · 消元提取。

use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    builder::PolynomialBuilder,
    canonical::canonicalize_polynomial,
    certificate::{GroebnerAlgorithm, GroebnerCertificate},
    coeff_kernel::CoeffRing,
    exponent::add_exponent_vectors,
    expr::Polynomial,
    ideal::Ideal,
    operations::sub_polynomial,
    order::MonomialOrder,
    ring::RingDescriptor,
    ring_table::RingTable,
};

/// Gröbner 计算资源合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroebnerLimits {
    /// 最大 S-pair 约化步数。
    pub max_s_pairs: u32,
    /// 最大基大小。
    pub max_basis_size: u32,
}

impl Default for GroebnerLimits {
    fn default() -> Self {
        Self { max_s_pairs: 10_000, max_basis_size: 128 }
    }
}

/// Gröbner 基结果。
#[derive(Debug, Clone, PartialEq)]
pub struct GroebnerBasis {
    /// 所属环。
    pub ring: RingId,
    /// 约化 Gröbner 基（同环 canonical）。
    pub basis: Vec<Polynomial>,
    /// 计算证书。
    pub certificate: GroebnerCertificate,
}

/// 计算 Gröbner 基（Buchberger；系数域须为域）。
pub fn compute_groebner_basis(generators: Vec<Polynomial>, rings: &RingTable, limits: GroebnerLimits) -> Result<GroebnerBasis> {
    let ideal = Ideal::new(generators)?;
    let desc = rings.get(ideal.ring).ok_or_else(|| ring_unknown(ideal.ring))?;
    let coeff = rings.coeff_kernel(ideal.ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "groebner_requires_field"));
    }
    let mut basis = normalize_generators(ideal.generators, rings)?;
    let input_count = basis.len();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..basis.len() {
        for j in (i + 1)..basis.len() {
            pairs.push((i, j));
        }
    }
    let mut steps = 0u32;
    let mut complete = true;
    while let Some((i, j)) = pairs.pop() {
        if steps >= limits.max_s_pairs {
            complete = false;
            break;
        }
        steps += 1;
        let s = s_polynomial(&basis[i], &basis[j], rings, &coeff)?;
        let remainder = reduce_polynomial(&s, &basis, rings, desc, &coeff)?;
        if remainder.terms.is_empty() {
            continue;
        }
        if basis.len() as u32 >= limits.max_basis_size {
            return Err(Diagnostic::new(DiagnosticCode::GroebnerResourceLimit)
                .detail("domain", "polynomial")
                .detail("operation", "basis_size"));
        }
        let idx = basis.len();
        basis.push(remainder);
        for k in 0..idx {
            pairs.push((k, idx));
        }
    }
    basis = autoreduce_basis(basis, rings, desc, &coeff)?;
    let certificate = GroebnerCertificate {
        algorithm: GroebnerAlgorithm::Buchberger,
        ring: ideal.ring,
        input_generators: input_count,
        basis_elements: basis.len(),
        s_pair_steps: steps,
        complete,
        elimination_elements: None,
    };
    Ok(GroebnerBasis { ring: ideal.ring, basis, certificate })
}

/// 消元理想：环须为 [`MonomialOrder::Elimination`]，返回消元块生成元。
pub fn compute_elimination_basis(
    generators: Vec<Polynomial>,
    rings: &RingTable,
    limits: GroebnerLimits,
) -> Result<GroebnerBasis> {
    let ideal = Ideal::new(generators)?;
    let desc = rings.get(ideal.ring).ok_or_else(|| ring_unknown(ideal.ring))?;
    let eliminate = match &desc.order {
        MonomialOrder::Elimination { eliminate, .. } => *eliminate as usize,
        _ => {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialOrderInvalid)
                .detail("domain", "polynomial")
                .detail("operation", "elimination_order_required"));
        }
    };
    let mut gb = compute_groebner_basis(ideal.generators, rings, limits)?;
    let elim = extract_elimination_polys(&gb.basis, eliminate);
    gb.certificate.elimination_elements = Some(elim.len());
    gb.basis = elim;
    Ok(gb)
}

/// 对理想成员做 Gröbner 约化（余式）。
pub fn reduce_ideal(polynomial: Polynomial, basis: &[Polynomial], rings: &RingTable) -> Result<Polynomial> {
    let desc = rings.get(polynomial.ring).ok_or_else(|| ring_unknown(polynomial.ring))?;
    let coeff = rings.coeff_kernel(polynomial.ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "reduce_requires_field"));
    }
    reduce_polynomial(&polynomial, basis, rings, desc, &coeff)
}

fn normalize_generators(gens: Vec<Polynomial>, rings: &RingTable) -> Result<Vec<Polynomial>> {
    let mut out = Vec::new();
    for g in gens {
        let c = canonicalize_polynomial(g, rings)?;
        if !c.terms.is_empty() {
            out.push(c);
        }
    }
    if out.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "groebner_zero_ideal"));
    }
    Ok(out)
}

fn leading_term(poly: &Polynomial) -> Option<super::expr::MonomialTerm> {
    poly.terms.first().cloned()
}

fn monomial_divides(divisor: &[u32], target: &[u32]) -> bool {
    divisor.iter().zip(target.iter()).all(|(&d, &t)| d <= t)
}

fn lcm_exponents(a: &[u32], b: &[u32]) -> Vec<u32> {
    a.iter().zip(b.iter()).map(|(&x, &y)| x.max(y)).collect()
}

fn exponents_delta(num: &[u32], den: &[u32]) -> Vec<u32> {
    num.iter().zip(den.iter()).map(|(&n, &d)| n - d).collect()
}

fn s_polynomial(f: &Polynomial, g: &Polynomial, rings: &RingTable, coeff: &CoeffRing<'_>) -> Result<Polynomial> {
    let lf = leading_term(f).ok_or_else(zero_poly_err)?;
    let lg = leading_term(g).ok_or_else(zero_poly_err)?;
    let lcm = lcm_exponents(&lf.exponents, &lg.exponents);
    let mult_f_exp = exponents_delta(&lcm, &lf.exponents);
    let mult_g_exp = exponents_delta(&lcm, &lg.exponents);
    let mf = multiply_by_monomial(f, coeff.inv(lf.coefficient.clone())?, &mult_f_exp, rings, coeff)?;
    let mg = multiply_by_monomial(g, coeff.inv(lg.coefficient.clone())?, &mult_g_exp, rings, coeff)?;
    sub_polynomial(mf, mg, rings)
}

fn multiply_by_monomial(
    poly: &Polynomial,
    scalar: Number,
    exp_delta: &[u32],
    rings: &RingTable,
    coeff: &CoeffRing<'_>,
) -> Result<Polynomial> {
    if poly.terms.is_empty() || scalar.is_zero() {
        return Ok(Polynomial::zero(poly.ring));
    }
    let mut b = PolynomialBuilder::new(poly.ring);
    for term in &poly.terms {
        let exponents = add_exponent_vectors(&term.exponents, exp_delta)?;
        let c = coeff.mul(scalar.clone(), term.coefficient.clone())?;
        b.push_term(c, exponents)?;
    }
    b.build(rings)
}

fn reduce_polynomial(
    poly: &Polynomial,
    basis: &[Polynomial],
    rings: &RingTable,
    _desc: &RingDescriptor,
    coeff: &CoeffRing<'_>,
) -> Result<Polynomial> {
    let mut remainder = poly.clone();
    loop {
        let lr = match leading_term(&remainder) {
            Some(t) => t,
            None => return Ok(remainder),
        };
        let mut reduced = false;
        for g in basis {
            let lg = match leading_term(g) {
                Some(t) => t,
                None => continue,
            };
            if !monomial_divides(&lg.exponents, &lr.exponents) {
                continue;
            }
            let delta = exponents_delta(&lr.exponents, &lg.exponents);
            let factor = coeff.div(lr.coefficient.clone(), lg.coefficient.clone())?;
            let term = multiply_by_monomial(g, factor, &delta, rings, coeff)?;
            remainder = sub_polynomial(remainder, term, rings)?;
            reduced = true;
            break;
        }
        if !reduced {
            return Ok(remainder);
        }
    }
}

fn autoreduce_basis(
    basis: Vec<Polynomial>,
    rings: &RingTable,
    desc: &RingDescriptor,
    coeff: &CoeffRing<'_>,
) -> Result<Vec<Polynomial>> {
    let mut out = Vec::new();
    for (i, g) in basis.iter().enumerate() {
        let others: Vec<Polynomial> = basis.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, p)| p.clone()).collect();
        let r = reduce_polynomial(g, &others, rings, desc, coeff)?;
        if r.terms.is_empty() {
            continue;
        }
        if out.iter().any(|p| leading_term(p).zip(leading_term(&r)).is_some_and(|(a, b)| a.exponents == b.exponents)) {
            continue;
        }
        out.push(r);
    }
    Ok(out)
}

fn extract_elimination_polys(basis: &[Polynomial], eliminate: usize) -> Vec<Polynomial> {
    basis.iter().filter(|p| p.terms.iter().all(|t| t.exponents.iter().take(eliminate).all(|&e| e == 0))).cloned().collect()
}

fn zero_poly_err() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "zero_polynomial")
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
