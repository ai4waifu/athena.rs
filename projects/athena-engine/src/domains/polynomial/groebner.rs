//! Gröbner 基（Buchberger）· 独立验证 · 类型分型结果 · 理想约化 · 消元。

use std::collections::HashSet;

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use crate::domains::algebra::{PropertyState, PropertyWitness};

use super::{
    builder::PolynomialBuilder,
    canonical::canonicalize_polynomial,
    certificate::{GroebnerAlgorithm, GroebnerCertificate, GroebnerStatus},
    coefficient_kernel::CoefficientRing,
    ideal::Ideal,
    monomial_layout::MonomialLayout,
    object::Polynomial,
    operations::sub_polynomial,
    order::MonomialOrder,
    ring_table::RingTable,
};
use crate::runtime::values::numeric_clone::clone_number;

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
///
/// `pending_pairs` / `pending_insertion` 使 Partial / ResourceLimited 可诚实恢复。
/// 索引相对于 `candidates` 当前顺序。恢复前不得对候选做会打乱下标的变换。
#[derive(Debug, PartialEq)]
pub struct GroebnerFrontier {
    /// 所属环。
    pub ring: RingId,
    /// 候选多项式（当前 Buchberger 基，未必自约化）。
    pub candidates: Vec<Polynomial>,
    /// 尚未处理的 critical pairs（下标相对 `candidates`）。
    pub pending_pairs: Vec<(usize, usize)>,
    /// 已算得但因 `max_basis_size` 未能插入的多项式。
    pub pending_insertion: Option<Polynomial>,
    /// 证书（`verified = false`）。
    pub certificate: GroebnerCertificate,
}

impl GroebnerFrontier {
    /// 候选切片。
    pub fn candidates(&self) -> &[Polynomial] {
        &self.candidates
    }

    /// 待处理 pair。
    pub fn pending_pairs(&self) -> &[(usize, usize)] {
        &self.pending_pairs
    }

    /// 是否仍有可恢复工作。
    pub fn has_resumable_work(&self) -> bool {
        self.pending_insertion.is_some() || !self.pending_pairs.is_empty()
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

/// 计算 Gröbner 基（Buchberger；系数域须为域）。
///
/// 仅 [`GroebnerComputation::Complete`] 可作 exact membership / 消元定理 / M-Graph exact witness。
pub fn compute_groebner_basis(generators: Vec<Polynomial>, rings: &RingTable, limits: GroebnerLimits) -> Result<GroebnerComputation> {
    let ideal = Ideal::new(generators)?;
    require_field_ring(ideal.ring, rings, "groebner_requires_field")?;
    let basis = normalize_generators(ideal.generators, rings)?;
    let input_count = basis.len();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..basis.len() {
        for j in (i + 1)..basis.len() {
            pairs.push((i, j));
        }
    }
    run_buchberger(ideal.ring, basis, pairs, None, input_count, 0, rings, limits)
}

/// 从 Partial / ResourceLimited frontier 恢复 Buchberger。
///
/// 恢复前重新校验环 / 域 / pair 下标。中间证书仍须经 `verify_groebner_basis` 才可 Complete。
pub fn resume_groebner_basis(frontier: GroebnerFrontier, rings: &RingTable, limits: GroebnerLimits) -> Result<GroebnerComputation> {
    require_field_ring(frontier.ring, rings, "groebner_resume_requires_field")?;
    if frontier.candidates.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "groebner_resume_empty_basis"));
    }
    for p in &frontier.candidates {
        if p.ring() != frontier.ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "groebner_resume_ring_mismatch"));
        }
    }
    if let Some(p) = &frontier.pending_insertion {
        if p.ring() != frontier.ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "groebner_resume_insertion_ring_mismatch"));
        }
    }
    let n = frontier.candidates.len();
    for &(i, j) in &frontier.pending_pairs {
        if i >= n || j >= n || i == j {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "polynomial")
                .detail("operation", "groebner_resume_invalid_pair")
                .detail("i", i.to_string())
                .detail("j", j.to_string())
                .detail("basis_len", n.to_string()));
        }
    }
    if !frontier.has_resumable_work() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "groebner_resume_no_pending_work"));
    }
    let input_count = frontier.certificate.input_generators.max(1);
    let prior_steps = frontier.certificate.s_pair_steps;
    run_buchberger(
        frontier.ring,
        frontier.candidates,
        frontier.pending_pairs,
        frontier.pending_insertion,
        input_count,
        prior_steps,
        rings,
        limits,
    )
}

fn run_buchberger(
    ring: RingId,
    mut basis: Vec<Polynomial>,
    pairs_in: Vec<(usize, usize)>,
    mut pending_insertion: Option<Polynomial>,
    input_count: usize,
    mut steps: u32,
    rings: &RingTable,
    limits: GroebnerLimits,
) -> Result<GroebnerComputation> {
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;
    let coeff = rings.coefficient_kernel(ring)?;
    let layout = &desc.monomial_layout;

    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut pending: HashSet<(usize, usize)> = HashSet::new();
    for (i, j) in pairs_in {
        enqueue_pair(i, j, &mut pairs, &mut pending);
    }

    if let Some(remainder) = pending_insertion.take() {
        if basis.len() as u32 >= limits.max_basis_size {
            return Ok(GroebnerComputation::ResourceLimited(frontier(
                ring,
                basis,
                pairs_from_pending(&pending),
                Some(remainder),
                input_count,
                steps,
                false,
                None,
            )));
        }
        let idx = basis.len();
        basis.push(remainder);
        for k in 0..idx {
            enqueue_pair(k, idx, &mut pairs, &mut pending);
        }
    }

    let mut truncated_pairs = false;
    let mut resource_limited = false;
    let mut deferred_insertion: Option<Polynomial> = None;
    while let Some((i, j)) = pairs.pop() {
        let key = ordered_pair(i, j);
        if !pending.remove(&key) {
            continue;
        }
        // Buchberger criterion 1: coprime leading monomials ⇒ S-pair reduces to 0.
        if leading_monomials_coprime(&basis[i], &basis[j]) {
            continue;
        }
        // Buchberger criterion 2 (chain): ∃k with LM(k)|lcm(LM(i),LM(j)) and pairs (i,k),(j,k) already treated.
        if chain_criterion_applies(&basis, i, j, &pending, layout)? {
            continue;
        }
        if steps >= limits.max_s_pairs {
            pending.insert(key);
            pairs.push(key);
            truncated_pairs = true;
            break;
        }
        steps = steps.saturating_add(1);
        let s = s_polynomial(&basis[i], &basis[j], rings, layout, &coeff)?;
        let remainder = reduce_polynomial(&s, &basis, rings, layout, &coeff)?;
        if remainder.terms().is_empty() {
            continue;
        }
        if basis.len() as u32 >= limits.max_basis_size {
            deferred_insertion = Some(remainder);
            resource_limited = true;
            break;
        }
        let idx = basis.len();
        basis.push(remainder);
        for k in 0..idx {
            enqueue_pair(k, idx, &mut pairs, &mut pending);
        }
    }

    if resource_limited {
        return Ok(GroebnerComputation::ResourceLimited(frontier(
            ring,
            basis,
            pairs_from_pending(&pending),
            deferred_insertion,
            input_count,
            steps,
            false,
            None,
        )));
    }
    if truncated_pairs {
        return Ok(GroebnerComputation::Partial(frontier(
            ring,
            basis,
            pairs_from_pending(&pending),
            None,
            input_count,
            steps,
            false,
            None,
        )));
    }

    basis = autoreduce_basis(basis, rings, layout, &coeff)?;
    let verification = verify_groebner_basis(&basis, rings)?;
    if !verification.all_s_pairs_reduce_to_zero {
        return Err(Diagnostic::new(DiagnosticCode::GroebnerVerificationFailed)
            .detail("domain", "polynomial")
            .detail("operation", "buchberger_post_verify"));
    }
    let certificate = GroebnerCertificate {
        algorithm: GroebnerAlgorithm::Buchberger,
        ring,
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
    Ok(GroebnerComputation::Complete(VerifiedGroebnerBasis { ring, basis, certificate, verification }))
}

fn ordered_pair(i: usize, j: usize) -> (usize, usize) {
    if i < j {
        (i, j)
    }
    else {
        (j, i)
    }
}

fn enqueue_pair(i: usize, j: usize, pairs: &mut Vec<(usize, usize)>, pending: &mut HashSet<(usize, usize)>) {
    if i == j {
        return;
    }
    let key = ordered_pair(i, j);
    if pending.insert(key) {
        pairs.push(key);
    }
}

fn pairs_from_pending(pending: &HashSet<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = pending.iter().copied().collect();
    out.sort_unstable();
    out
}

/// Buchberger chain criterion: ∃`k` s.t. `LM(bk) | lcm(LM(bi), LM(bj))` and pairs `(i,k)`, `(j,k)` already treated.
fn chain_criterion_applies(
    basis: &[Polynomial],
    i: usize,
    j: usize,
    pending: &HashSet<(usize, usize)>,
    layout: &MonomialLayout,
) -> Result<bool> {
    let Some(li) = basis[i].terms().first()
    else {
        return Ok(false);
    };
    let Some(lj) = basis[j].terms().first()
    else {
        return Ok(false);
    };
    let lcm_ij = layout.lcm_exponents(li.exponents(), lj.exponents())?;
    let lcm_packed = layout.pack(&lcm_ij)?;
    for k in 0..basis.len() {
        if k == i || k == j {
            continue;
        }
        let Some(lk) = basis[k].terms().first()
        else {
            continue;
        };
        let lk_packed = layout.pack(lk.exponents())?;
        if !layout.packed_divides(&lk_packed, &lcm_packed)? {
            continue;
        }
        if !pending.contains(&ordered_pair(i, k)) && !pending.contains(&ordered_pair(j, k)) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn require_field_ring(ring: RingId, rings: &RingTable, operation: &str) -> Result<()> {
    let coeff = rings.coefficient_kernel(ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", operation));
    }
    Ok(())
}

/// 消元理想：须为完整已验证 Gröbner 基；环须为 [`MonomialOrder::Elimination`]。
pub fn compute_elimination_basis(generators: Vec<Polynomial>, rings: &RingTable, limits: GroebnerLimits) -> Result<GroebnerComputation> {
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
            Ok(GroebnerComputation::Complete(VerifiedGroebnerBasis { ring: verified.ring, basis: elim, certificate, verification }))
        }
        GroebnerComputation::Partial(mut frontier) => {
            // 消元过滤会打乱 pair 下标，incomplete 结果不可再 resume。
            frontier.candidates = extract_elimination_polys(&frontier.candidates, eliminate);
            frontier.pending_pairs.clear();
            frontier.pending_insertion = None;
            frontier.certificate.basis_elements = frontier.candidates.len();
            frontier.certificate.elimination_elements = Some(frontier.candidates.len());
            frontier.certificate.mark_unverified();
            Ok(GroebnerComputation::Partial(frontier))
        }
        GroebnerComputation::ResourceLimited(mut frontier) => {
            frontier.candidates = extract_elimination_polys(&frontier.candidates, eliminate);
            frontier.pending_pairs.clear();
            frontier.pending_insertion = None;
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
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch).detail("domain", "polynomial").detail("operation", "reduce_ring_mismatch"));
    }
    if !basis.certificate.is_exact_witness() {
        return Err(Diagnostic::new(DiagnosticCode::GroebnerIncomplete)
            .detail("domain", "polynomial")
            .detail("operation", "reduce_requires_verified"));
    }
    let desc = rings.get(polynomial.ring()).ok_or_else(|| ring_unknown(polynomial.ring()))?;
    let coeff = rings.coefficient_kernel(polynomial.ring())?;
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
    let coeff = rings.coefficient_kernel(polynomial.ring())?;
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
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "verify_empty_basis"));
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
    let coeff = rings.coefficient_kernel(ring)?;
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
    pending_pairs: Vec<(usize, usize)>,
    pending_insertion: Option<Polynomial>,
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
    GroebnerFrontier { ring, candidates, pending_pairs, pending_insertion, certificate }
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
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "groebner_zero_ideal"));
    }
    Ok(out)
}

fn leading_term(poly: &Polynomial) -> Option<super::object::MonomialTerm> {
    poly.terms().first().map(|t| t.owning_copy())
}

/// Buchberger first criterion: `LM(f)` and `LM(g)` coprime ⇒ `S(f,g)` → 0.
fn leading_monomials_coprime(f: &Polynomial, g: &Polynomial) -> bool {
    let Some(lf) = f.terms().first()
    else {
        return false;
    };
    let Some(lg) = g.terms().first()
    else {
        return false;
    };
    if lf.exponents().len() != lg.exponents().len() {
        return false;
    }
    lf.exponents().iter().zip(lg.exponents().iter()).all(|(a, b)| *a == 0 || *b == 0)
}

fn s_polynomial(f: &Polynomial, g: &Polynomial, rings: &RingTable, layout: &MonomialLayout, coeff: &CoefficientRing<'_>) -> Result<Polynomial> {
    let lf = leading_term(f).ok_or_else(zero_poly_err)?;
    let lg = leading_term(g).ok_or_else(zero_poly_err)?;
    let lcm = layout.lcm_exponents(&lf.exponents, &lg.exponents)?;
    let mult_f_exp = layout.exponents_delta(&lcm, &lf.exponents)?;
    let mult_g_exp = layout.exponents_delta(&lcm, &lg.exponents)?;
    let mf = multiply_by_monomial(f, coeff.inv(clone_number(&lf.coefficient))?, &mult_f_exp, layout, rings, coeff)?;
    let mg = multiply_by_monomial(g, coeff.inv(clone_number(&lg.coefficient))?, &mult_g_exp, layout, rings, coeff)?;
    sub_polynomial(mf, mg, rings)
}

fn multiply_by_monomial(
    poly: &Polynomial,
    scalar: athena_numeric::Number,
    exp_delta: &[u32],
    layout: &MonomialLayout,
    rings: &RingTable,
    coeff: &CoefficientRing<'_>,
) -> Result<Polynomial> {
    if poly.terms().is_empty() || scalar.is_zero() {
        return Ok(Polynomial::zero(poly.ring()));
    }
    let mut b = PolynomialBuilder::new(poly.ring());
    for term in poly.terms() {
        let exponents = layout.add_exponents(term.exponents(), exp_delta)?;
        let c = coeff.mul(clone_number(&scalar), clone_number(term.coefficient()))?;
        b.push_term(c, exponents)?;
    }
    b.build(rings)
}

fn reduce_polynomial(
    poly: &Polynomial,
    basis: &[Polynomial],
    rings: &RingTable,
    layout: &MonomialLayout,
    coeff: &CoefficientRing<'_>,
) -> Result<Polynomial> {
    let mut remainder = poly.owning_copy();
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
            let factor = coeff.div(clone_number(&lr.coefficient), clone_number(&lg.coefficient))?;
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
    coeff: &CoefficientRing<'_>,
) -> Result<Vec<Polynomial>> {
    let mut out = Vec::new();
    for (i, g) in basis.iter().enumerate() {
        let others: Vec<Polynomial> = basis.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, p)| p.owning_copy()).collect();
        let r = reduce_polynomial(g, &others, rings, layout, coeff)?;
        if r.terms.is_empty() {
            continue;
        }
        let r_leading = layout.pack(&r.terms[0].exponents)?;
        if out.iter().any(|p| {
            leading_term(p).and_then(|lt| layout.pack(lt.exponents()).ok()).is_some_and(|lt_packed| layout.packed_equal(&lt_packed, &r_leading))
        }) {
            continue;
        }
        out.push(r);
    }
    Ok(out)
}

fn extract_elimination_polys(basis: &[Polynomial], eliminate: usize) -> Vec<Polynomial> {
    basis.iter().filter(|p| p.terms().iter().all(|t| t.exponents().iter().take(eliminate).all(|&e| e == 0))).map(|p| p.owning_copy()).collect()
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

#[cfg(test)]
mod criterion_tests {
    use super::*;
    use crate::domains::polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, RingTable};
    use athena_numeric::Number;
    use athena_types::SymbolId;

    fn poly(rings: &RingTable, ring: RingId, terms: &[(i64, Vec<u32>)]) -> Polynomial {
        let mut b = PolynomialBuilder::new(ring);
        for &(c, ref exp) in terms {
            b.push_term(Number::small_int(c), exp.clone()).unwrap();
        }
        b.build(rings).unwrap()
    }

    #[test]
    fn chain_criterion_true_when_third_lm_divides_lcm_and_pairs_treated() {
        let mut rings = RingTable::new();
        let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
        let layout = &rings.get(ring).unwrap().monomial_layout;
        // LM: x^2, y^2, xy — xy | lcm(x^2,y^2)=x^2y^2
        let basis = vec![
            poly(&rings, ring, &[(1, vec![2, 0])]),
            poly(&rings, ring, &[(1, vec![0, 2])]),
            poly(&rings, ring, &[(1, vec![1, 1])]),
        ];
        let pending = HashSet::new();
        assert!(chain_criterion_applies(&basis, 0, 1, &pending, layout).unwrap());
    }

    #[test]
    fn chain_criterion_false_while_side_pairs_still_pending() {
        let mut rings = RingTable::new();
        let ring = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0), SymbolId(1)], MonomialOrder::Lex).unwrap();
        let layout = &rings.get(ring).unwrap().monomial_layout;
        let basis = vec![
            poly(&rings, ring, &[(1, vec![2, 0])]),
            poly(&rings, ring, &[(1, vec![0, 2])]),
            poly(&rings, ring, &[(1, vec![1, 1])]),
        ];
        let mut pending = HashSet::new();
        pending.insert(ordered_pair(0, 2));
        pending.insert(ordered_pair(1, 2));
        assert!(!chain_criterion_applies(&basis, 0, 1, &pending, layout).unwrap());
    }
}
