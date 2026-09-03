//! Gröbner 基（Buchberger）· 独立验证 · 类型分型结果 · 理想约化 · 消元。

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use crate::algebra::{PropertyState, PropertyWitness};

use super::{
    builder::PolynomialBuilder,
    canonical::canonicalize_polynomial,
    certificate::{GroebnerAlgorithm, GroebnerCertificate, GroebnerStatus},
    coeff_kernel::CoeffRing,
    expr::Polynomial,
    ideal::Ideal,
    monomial_layout::MonomialLayout,
    operations::sub_polynomial,
    order::MonomialOrder,
    ring_table::RingTable,
};

/// Gröbner 计算资源合同。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

/// 独立验证报告。
#[derive(Debug, PartialEq, Eq)]
pub struct GroebnerVerificationReport {
    /// 所属环。
    pub ring: RingId,
    /// 检查的 critical pair 数。
    pub pairs_checked: u32,
    /// 是否全部 S-pair 约化为零。
    pub all_s_pairs_reduce_to_zero: bool,
}

/// 已验证的完整 Gröbner 基（唯一允许 membership / 规范余式 / 消元定理的证书对象）。
#[derive(Debug, PartialEq)]
pub struct VerifiedGroebnerBasis {
    /// 所属环。
    pub ring: RingId,
    /// 基元素（canonical）。
    pub basis: Vec<Polynomial>,
    /// 证书（`complete && verified`）。
    pub certificate: GroebnerCertificate,
    /// 验证报告。
    pub verification: GroebnerVerificationReport,
}

impl VerifiedGroebnerBasis {
    /// 基切片。
    pub fn basis(&self) -> &[Polynomial] {
        &self.basis
    }
}

/// 未完成计算的候选前沿（不可作数学证书）。
#[derive(Debug, PartialEq)]
pub struct GroebnerFrontier {
    /// 所属环。
    pub ring: RingId,
    /// 候选多项式。
    pub candidates: Vec<Polynomial>,
    /// 证书（`verified = false`）。
    pub certificate: GroebnerCertificate,
}

impl GroebnerFrontier {
    /// 候选切片。
    pub fn candidates(&self) -> &[Polynomial] {
        &self.candidates
    }
}

/// Gröbner 计算结果的显式状态分型。
#[derive(Debug, PartialEq)]
pub enum GroebnerComputation {
    /// 完成且独立验证通过。
    Complete(VerifiedGroebnerBasis),
    /// S-pair 预算耗尽。
    Partial(GroebnerFrontier),
    /// 基大小等资源硬上限。
    ResourceLimited(GroebnerFrontier),
}

impl GroebnerComputation {
    /// 稳定状态标签。
    pub fn status(&self) -> GroebnerStatus {
        match self {
            Self::Complete(_) => GroebnerStatus::Verified,
            Self::Partial(_) => GroebnerStatus::Partial,
            Self::ResourceLimited(_) => GroebnerStatus::ResourceLimited,
        }
    }

    /// 是否为已验证完整基。
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    /// 已验证基（若有）。
    pub fn as_verified(&self) -> Option<&VerifiedGroebnerBasis> {
        match self {
            Self::Complete(v) => Some(v),
            _ => None,
        }
    }

    /// 环 id。
    pub fn ring(&self) -> RingId {
        match self {
            Self::Complete(v) => v.ring,
            Self::Partial(f) | Self::ResourceLimited(f) => f.ring,
        }
    }

    /// 基或候选多项式。
    pub fn polynomials(&self) -> &[Polynomial] {
        match self {
            Self::Complete(v) => v.basis(),
            Self::Partial(f) | Self::ResourceLimited(f) => f.candidates(),
        }
    }

    /// 证书。
    pub fn certificate(&self) -> &GroebnerCertificate {
        match self {
            Self::Complete(v) => &v.certificate,
            Self::Partial(f) | Self::ResourceLimited(f) => &f.certificate,
        }
    }
}

/// 兼容旧名称：完整计算的已验证基。
pub type GroebnerBasis = VerifiedGroebnerBasis;

/// 计算 Gröbner 基（Buchberger；系数域须为域）。
///
/// 仅 [`GroebnerComputation::Complete`] 可作 exact membership / 消元定理 / M-Graph exact witness。
pub fn compute_groebner_basis(
    generators: Vec<Polynomial>,
    rings: &RingTable,
    limits: GroebnerLimits,
) -> Result<GroebnerComputation> {
    let ideal = Ideal::new(generators)?;
    let desc = rings.get(ideal.ring).ok_or_else(|| ring_unknown(ideal.ring))?;
    let coeff = rings.coeff_kernel(ideal.ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "groebner_requires_field"));
    }
    let layout = &desc.monomial_layout;
    let mut basis = normalize_generators(ideal.generators, rings)?;
    let input_count = basis.len();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..basis.len() {
        for j in (i + 1)..basis.len() {
            pairs.push((i, j));
        }
    }
    let mut steps = 0u32;
    let mut truncated_pairs = false;
    let mut resource_limited = false;
    while let Some((i, j)) = pairs.pop() {
        if steps >= limits.max_s_pairs {
            truncated_pairs = true;
            break;
        }
        steps += 1;
        let s = s_polynomial(&basis[i], &basis[j], rings, layout, &coeff)?;
        let remainder = reduce_polynomial(&s, &basis, rings, layout, &coeff)?;
        if remainder.terms().is_empty() {
            continue;
        }
        if basis.len() as u32 >= limits.max_basis_size {
            resource_limited = true;
            break;
        }
        let idx = basis.len();
        basis.push(remainder);
        for k in 0..idx {
            pairs.push((k, idx));
        }
    }
    basis = autoreduce_basis(basis, rings, layout, &coeff)?;
    if resource_limited {
        return Ok(GroebnerComputation::ResourceLimited(frontier(ideal.ring, basis, input_count, steps, false, None)));
    }
    if truncated_pairs {
        return Ok(GroebnerComputation::Partial(frontier(ideal.ring, basis, input_count, steps, false, None)));
    }
    let verification = verify_groebner_basis(&basis, rings)?;
    if !verification.all_s_pairs_reduce_to_zero {
        return Err(Diagnostic::new(DiagnosticCode::GroebnerVerificationFailed)
            .detail("domain", "polynomial")
            .detail("operation", "buchberger_post_verify"));
    }
    let certificate = GroebnerCertificate {
        algorithm: GroebnerAlgorithm::Buchberger,
        ring: ideal.ring,
        input_generators: input_count,
        basis_elements: basis.len(),
        s_pair_steps: steps,
        complete: true,
        verification: PropertyState::Proven {
            value: (),
            witness: PropertyWitness::placeholder("groebner_independent_verifier"),
        },
        elimination_elements: None,
    };
    Ok(GroebnerComputation::Complete(VerifiedGroebnerBasis { ring: ideal.ring, basis, certificate, verification }))
}

/// 消元理想：须为完整已验证 Gröbner 基；环须为 [`MonomialOrder::Elimination`]。
pub fn compute_elimination_basis(
    generators: Vec<Polynomial>,
    rings: &RingTable,
    limits: GroebnerLimits,
) -> Result<GroebnerComputation> {
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
    let computation = compute_groebner_basis(ideal.generators, rings, limits)?;
    match computation {
        GroebnerComputation::Complete(verified) => {
            let elim = extract_elimination_polys(&verified.basis, eliminate);
            let verification = verify_groebner_basis(&elim, rings)?;
            if !verification.all_s_pairs_reduce_to_zero {
                return Err(Diagnostic::new(DiagnosticCode::GroebnerVerificationFailed)
                    .detail("domain", "polynomial")
                    .detail("operation", "elimination_post_verify"));
            }
            let mut certificate = verified.certificate;
            certificate.basis_elements = elim.len();
            certificate.elimination_elements = Some(elim.len());
            certificate.mark_verified();
            certificate.complete = true;
            Ok(GroebnerComputation::Complete(VerifiedGroebnerBasis {
                ring: verified.ring,
                basis: elim,
                certificate,
                verification,
            }))
        }
        GroebnerComputation::Partial(mut frontier) => {
            frontier.candidates = extract_elimination_polys(&frontier.candidates, eliminate);
            frontier.certificate.basis_elements = frontier.candidates.len();
            frontier.certificate.elimination_elements = Some(frontier.candidates.len());
            frontier.certificate.mark_unverified();
            Ok(GroebnerComputation::Partial(frontier))
        }
        GroebnerComputation::ResourceLimited(mut frontier) => {
            frontier.candidates = extract_elimination_polys(&frontier.candidates, eliminate);
            frontier.certificate.basis_elements = frontier.candidates.len();
            frontier.certificate.elimination_elements = Some(frontier.candidates.len());
            frontier.certificate.mark_unverified();
            Ok(GroebnerComputation::ResourceLimited(frontier))
        }
    }
}

/// 对已验证 Gröbner 基做规范余式（strict API）。
pub fn reduce_by_verified(polynomial: Polynomial, basis: &VerifiedGroebnerBasis, rings: &RingTable) -> Result<Polynomial> {
    if polynomial.ring() != basis.ring {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "reduce_ring_mismatch"));
    }
    if !basis.certificate.is_exact_witness() {
        return Err(Diagnostic::new(DiagnosticCode::GroebnerIncomplete)
            .detail("domain", "polynomial")
            .detail("operation", "reduce_requires_verified"));
    }
    let desc = rings.get(polynomial.ring()).ok_or_else(|| ring_unknown(polynomial.ring()))?;
    let coeff = rings.coeff_kernel(polynomial.ring())?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "reduce_requires_field"));
    }
    let layout = &desc.monomial_layout;
    reduce_polynomial(&polynomial, &basis.basis, rings, layout, &coeff)
}

/// 理想成员判定：余式为零当且仅当（在已验证基下）属于理想。
pub fn ideal_membership(polynomial: Polynomial, basis: &VerifiedGroebnerBasis, rings: &RingTable) -> Result<bool> {
    let rem = reduce_by_verified(polynomial, basis, rings)?;
    Ok(rem.terms.is_empty())
}

/// 启发式约化（接受任意生成元列表；**不可**作规范余式 / membership 证书）。
///
/// 严格路径请用 [`reduce_by_verified`]。
pub fn reduce_ideal(polynomial: Polynomial, basis: &[Polynomial], rings: &RingTable) -> Result<Polynomial> {
    let desc = rings.get(polynomial.ring()).ok_or_else(|| ring_unknown(polynomial.ring()))?;
    let coeff = rings.coeff_kernel(polynomial.ring())?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "reduce_requires_field"));
    }
    let layout = &desc.monomial_layout;
    reduce_polynomial(&polynomial, basis, rings, layout, &coeff)
}

/// 独立验证：所有 critical S-pair 约化为零。
pub fn verify_groebner_basis(basis: &[Polynomial], rings: &RingTable) -> Result<GroebnerVerificationReport> {
    if basis.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "verify_empty_basis"));
    }
    let ring = basis[0].ring();
    for p in basis {
        if p.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "verify_ring_mismatch"));
        }
    }
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    let coeff = rings.coeff_kernel(ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "verify_requires_field"));
    }
    let layout = &desc.monomial_layout;
    let mut pairs_checked = 0u32;
    for i in 0..basis.len() {
        for j in (i + 1)..basis.len() {
            pairs_checked = pairs_checked.saturating_add(1);
            let s = s_polynomial(&basis[i], &basis[j], rings, layout, &coeff)?;
            let rem = reduce_polynomial(&s, basis, rings, layout, &coeff)?;
            if !rem.terms.is_empty() {
                return Ok(GroebnerVerificationReport { ring, pairs_checked, all_s_pairs_reduce_to_zero: false });
            }
        }
    }
    Ok(GroebnerVerificationReport { ring, pairs_checked, all_s_pairs_reduce_to_zero: true })
}

fn frontier(
    ring: RingId,
    candidates: Vec<Polynomial>,
    input_generators: usize,
    steps: u32,
    complete: bool,
    elimination_elements: Option<usize>,
) -> GroebnerFrontier {
    let certificate = GroebnerCertificate {
        algorithm: GroebnerAlgorithm::Buchberger,
        ring,
        input_generators,
        basis_elements: candidates.len(),
        s_pair_steps: steps,
        complete,
        verification: PropertyState::Unknown,
        elimination_elements,
    };
    GroebnerFrontier { ring, candidates, certificate }
}

fn normalize_generators(gens: Vec<Polynomial>, rings: &RingTable) -> Result<Vec<Polynomial>> {
    let mut out = Vec::new();
    for g in gens {
        let c = canonicalize_polynomial(g, rings)?;
        if !c.terms().is_empty() {
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
    poly.terms().first().cloned()
}

fn s_polynomial(
    f: &Polynomial,
    g: &Polynomial,
    rings: &RingTable,
    layout: &MonomialLayout,
    coeff: &CoeffRing<'_>,
) -> Result<Polynomial> {
    let lf = leading_term(f).ok_or_else(zero_poly_err)?;
    let lg = leading_term(g).ok_or_else(zero_poly_err)?;
    let lcm = layout.lcm_exponents(&lf.exponents, &lg.exponents)?;
    let mult_f_exp = layout.exponents_delta(&lcm, &lf.exponents)?;
    let mult_g_exp = layout.exponents_delta(&lcm, &lg.exponents)?;
    let mf = multiply_by_monomial(f, coeff.inv(lf.coefficient.clone())?, &mult_f_exp, layout, rings, coeff)?;
    let mg = multiply_by_monomial(g, coeff.inv(lg.coefficient.clone())?, &mult_g_exp, layout, rings, coeff)?;
    sub_polynomial(mf, mg, rings)
}

fn multiply_by_monomial(
    poly: &Polynomial,
    scalar: athena_numeric::Number,
    exp_delta: &[u32],
    layout: &MonomialLayout,
    rings: &RingTable,
    coeff: &CoeffRing<'_>,
) -> Result<Polynomial> {
    if poly.terms().is_empty() || scalar.is_zero() {
        return Ok(Polynomial::zero(poly.ring()));
    }
    let mut b = PolynomialBuilder::new(poly.ring());
    for term in poly.terms() {
        let exponents = layout.add_exponents(term.exponents(), exp_delta)?;
        let c = coeff.mul(scalar.clone(), term.coefficient().clone())?;
        b.push_term(c, exponents)?;
    }
    b.build(rings)
}

fn reduce_polynomial(
    poly: &Polynomial,
    basis: &[Polynomial],
    rings: &RingTable,
    layout: &MonomialLayout,
    coeff: &CoeffRing<'_>,
) -> Result<Polynomial> {
    let mut remainder = poly.clone();
    loop {
        let lr = match leading_term(&remainder) {
            Some(t) => t,
            None => return Ok(remainder),
        };
        let lr_packed = layout.pack(&lr.exponents)?;
        let mut reduced = false;
        for g in basis {
            let lg = match leading_term(g) {
                Some(t) => t,
                None => continue,
            };
            let lg_packed = layout.pack(&lg.exponents)?;
            if !layout.packed_divides(&lg_packed, &lr_packed)? {
                continue;
            }
            let delta = layout.exponents_delta(&lr.exponents, &lg.exponents)?;
            let factor = coeff.div(lr.coefficient.clone(), lg.coefficient.clone())?;
            let term = multiply_by_monomial(g, factor, &delta, layout, rings, coeff)?;
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
    layout: &MonomialLayout,
    coeff: &CoeffRing<'_>,
) -> Result<Vec<Polynomial>> {
    let mut out = Vec::new();
    for (i, g) in basis.iter().enumerate() {
        let others: Vec<Polynomial> = basis.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, p)| p.clone()).collect();
        let r = reduce_polynomial(g, &others, rings, layout, coeff)?;
        if r.terms.is_empty() {
            continue;
        }
        let r_leading = layout.pack(&r.terms[0].exponents)?;
        if out.iter().any(|p| {
            leading_term(p)
                .and_then(|lt| layout.pack(lt.exponents()).ok())
                .is_some_and(|lt_packed| layout.packed_equal(&lt_packed, &r_leading))
        }) {
            continue;
        }
        out.push(r);
    }
    Ok(out)
}

fn extract_elimination_polys(basis: &[Polynomial], eliminate: usize) -> Vec<Polynomial> {
    basis.iter().filter(|p| p.terms().iter().all(|t| t.exponents().iter().take(eliminate).all(|&e| e == 0))).cloned().collect()
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
