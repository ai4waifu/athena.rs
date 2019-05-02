//! 多项式因式分解（骨架合同 · Living `08`）。
//!
//! 完整性模型对齐数论 [`crate::number_theory::Factorization`]：禁止裸 `Vec<Polynomial>`。
//! 常数 / 一次完整；ℚ/ℤ 上有理根试除 + 二次判别式；更高未裂余式显式 `Partial`。

use athena_numeric::{Integer, Number, Rational, add as num_add, div as num_div, mul as num_mul, neg as num_neg};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use crate::number_theory::isqrt_if_exact;
use crate::numeric_clone::{clone_integer, clone_number, clone_rational};
use super::{
    builder::PolynomialBuilder,
    canonical::canonicalize_polynomial,
    expr::Polynomial,
    ring::{CoefficientDomain, DivisionPolicy},
    ring_table::RingTable,
    univariate::div_univariate,
};

/// 多项式因式分解资源合同。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PolynomialFactorLimits {
    /// 允许分解的最大次数（超过则 `ResourceLimited` + `input_rejected`）。
    pub max_degree: u32,
    /// 最大算法步数（骨架阶段仅占位）。
    pub max_steps: u32,
}

impl Default for PolynomialFactorLimits {
    fn default() -> Self {
        Self { max_degree: 64, max_steps: 10_000 }
    }
}

/// 单个多项式因子的不可约状态。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PolynomialFactorStatus {
    /// 已证明不可约（骨架：仅一次多项式）。
    ProvenIrreducible,
    /// 概率不可约（算法路径预留）。
    ProbableIrreducible,
    /// 尚未判定（不得冒充 complete）。
    Unknown,
}

/// 余因子状态。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PolynomialCofactorStatus {
    /// 完全分解，余因子为单位（常数 1，表示已吸收进 `unit`）。
    One,
    /// 仍有未分解的高次余式。
    Unsplit,
    /// 素性 / 不可约性未决。
    Unknown,
}

/// 多项式因式分解完整性（由组件推导，不单独存矛盾字段）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PolynomialFactorizationCompleteness {
    /// 完全分解为已证明不可约因子。
    Complete,
    /// 余因子吸收完毕，但存在概率不可约因子。
    Probable,
    /// 仍有未分解余式。
    Partial,
    /// 触及资源 / 输入拒绝上限。
    ResourceLimited,
}

/// 单个多项式因子（底 × 指数）。
#[derive(Debug, PartialEq)]
pub struct PolynomialFactorComponent {
    /// 因子底（canonical 非单位多项式）。
    pub base: Polynomial,
    /// 指数（`> 0`）。
    pub exponent: u32,
    /// 不可约状态。
    pub status: PolynomialFactorStatus,
}

/// 带完备性分型的多项式因式分解结果。
#[derive(Debug, PartialEq)]
pub struct PolynomialFactorization {
    /// 所属环。
    pub ring: RingId,
    /// 单位 / 内容（常数系数；骨架阶段为有理/整数常数）。
    pub unit: Number,
    /// 已抽出的因子。
    pub factors: Vec<PolynomialFactorComponent>,
    /// 未完全分解的余式（完全分解时为零多项式）。
    pub cofactor: Polynomial,
    /// 余因子状态。
    pub cofactor_status: PolynomialCofactorStatus,
    /// 是否因次数等输入上限被拒绝。
    pub input_rejected: bool,
    /// 是否因算法预算耗尽而停止。
    pub resource_exhausted: bool,
}

impl PolynomialFactorComponent {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            base: self.base.owning_copy(),
            exponent: self.exponent,
            status: self.status,
        }
    }
}

impl Clone for PolynomialFactorComponent {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

impl PolynomialFactorization {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            ring: self.ring,
            unit: clone_number(&self.unit),
            factors: self.factors.iter().map(PolynomialFactorComponent::owning_copy).collect(),
            cofactor: self.cofactor.owning_copy(),
            cofactor_status: self.cofactor_status,
            input_rejected: self.input_rejected,
            resource_exhausted: self.resource_exhausted,
        }
    }

    /// 由组件推导整体完整性。
    pub fn completeness(&self) -> PolynomialFactorizationCompleteness {
        if self.input_rejected || self.resource_exhausted {
            return PolynomialFactorizationCompleteness::ResourceLimited;
        }
        let has_probable = self.factors.iter().any(|c| c.status == PolynomialFactorStatus::ProbableIrreducible);
        let all_proven = self.factors.iter().all(|c| c.status == PolynomialFactorStatus::ProvenIrreducible);
        match self.cofactor_status {
            PolynomialCofactorStatus::One if all_proven && !has_probable => PolynomialFactorizationCompleteness::Complete,
            PolynomialCofactorStatus::One if has_probable => PolynomialFactorizationCompleteness::Probable,
            PolynomialCofactorStatus::Unsplit | PolynomialCofactorStatus::Unknown => {
                PolynomialFactorizationCompleteness::Partial
            }
            PolynomialCofactorStatus::One => PolynomialFactorizationCompleteness::Partial,
        }
    }

    /// 是否可作为 M-Graph exact witness。
    pub fn is_exact_witness(&self) -> bool {
        self.completeness() == PolynomialFactorizationCompleteness::Complete
    }
}

impl Clone for PolynomialFactorization {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

/// 单变量因式分解。
///
/// - 零多项式 → 域错误
/// - 常数 → `Complete`（仅 `unit`）
/// - 一次 → `Complete`（`ProvenIrreducible`）
/// - ℚ/ℤ：有理根试除；二次无有理根且判别式非平方 → 不可约 `Complete`
/// - 更高未裂余式 → `Partial`（cofactor）
/// - `deg > max_degree` → `ResourceLimited` + `input_rejected`
pub fn factor_univariate(
    polynomial: Polynomial,
    rings: &RingTable,
    limits: PolynomialFactorLimits,
) -> Result<PolynomialFactorization> {
    let poly = canonicalize_polynomial(polynomial, rings)?;
    let ring = poly.ring();
    let desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;

    if poly.terms().is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "factor_zero_polynomial"));
    }

    let deg = total_degree_univariate(&poly)?;
    if deg > limits.max_degree {
        return Ok(PolynomialFactorization {
            ring,
            unit: Number::small_int(1),
            factors: Vec::new(),
            cofactor: poly,
            cofactor_status: PolynomialCofactorStatus::Unsplit,
            input_rejected: true,
            resource_exhausted: false,
        });
    }

    if deg == 0 {
        let unit = clone_number(&poly.terms()[0].coefficient);
        return Ok(PolynomialFactorization {
            ring,
            unit,
            factors: Vec::new(),
            cofactor: Polynomial::zero(ring),
            cofactor_status: PolynomialCofactorStatus::One,
            input_rejected: false,
            resource_exhausted: false,
        });
    }

    if deg == 1 {
        return Ok(PolynomialFactorization {
            ring,
            unit: Number::small_int(1),
            factors: vec![PolynomialFactorComponent {
                base: poly,
                exponent: 1,
                status: PolynomialFactorStatus::ProvenIrreducible,
            }],
            cofactor: Polynomial::zero(ring),
            cofactor_status: PolynomialCofactorStatus::One,
            input_rejected: false,
            resource_exhausted: false,
        });
    }

    let domain = rings.coefficient_domain_for_descriptor(desc).ok_or_else(|| ring_unknown(ring))?;
    match domain {
        CoefficientDomain::Rational | CoefficientDomain::Integer => factor_rational_roots(poly, rings, limits.max_steps),
        _ => Ok(PolynomialFactorization {
            ring,
            unit: Number::small_int(1),
            factors: Vec::new(),
            cofactor: poly,
            cofactor_status: PolynomialCofactorStatus::Unsplit,
            input_rejected: false,
            resource_exhausted: false,
        }),
    }
}

/// ℚ/ℤ 有理根试除 + 二次判别式。
fn factor_rational_roots(
    mut poly: Polynomial,
    rings: &RingTable,
    max_steps: u32,
) -> Result<PolynomialFactorization> {
    let ring = poly.ring();
    let mut unit = Number::small_int(1);
    let mut factors = Vec::new();
    let mut steps = 0u32;

    loop {
        steps = steps.saturating_add(1);
        if steps > max_steps {
            return Ok(PolynomialFactorization {
                ring,
                unit,
                factors,
                cofactor: poly,
                cofactor_status: PolynomialCofactorStatus::Unsplit,
                input_rejected: false,
                resource_exhausted: true,
            });
        }

        let deg = total_degree_univariate(&poly)?;
        if deg == 0 {
            if let Some(c) = poly.terms().first() {
                unit = num_mul(unit, clone_number(c.coefficient()))?;
            }
            return Ok(PolynomialFactorization {
                ring,
                unit,
                factors,
                cofactor: Polynomial::zero(ring),
                cofactor_status: PolynomialCofactorStatus::One,
                input_rejected: false,
                resource_exhausted: false,
            });
        }
        if deg == 1 {
            factors.push(PolynomialFactorComponent {
                base: poly,
                exponent: 1,
                status: PolynomialFactorStatus::ProvenIrreducible,
            });
            return Ok(PolynomialFactorization {
                ring,
                unit,
                factors,
                cofactor: Polynomial::zero(ring),
                cofactor_status: PolynomialCofactorStatus::One,
                input_rejected: false,
                resource_exhausted: false,
            });
        }

        if let Some(root) = find_rational_root(&poly)? {
            let linear = linear_factor_for_root(ring, rings, &root)?;
            let div = div_univariate(poly.owning_copy(), linear.owning_copy(), DivisionPolicy::FieldDivision, rings)?;
            if !div.remainder.terms().is_empty() {
                return Err(Diagnostic::new(DiagnosticCode::DomainError)
                    .detail("domain", "polynomial")
                    .detail("operation", "rational_root_division")
                    .detail("reason", "nonzero_remainder"));
            }
            factors.push(PolynomialFactorComponent {
                base: linear,
                exponent: 1,
                status: PolynomialFactorStatus::ProvenIrreducible,
            });
            poly = div.quotient;
            continue;
        }

        if deg == 2 {
            match split_quadratic_over_rationals(&poly, rings)? {
                QuadraticSplit::TwoLinears(f1, f2) => {
                    factors.push(PolynomialFactorComponent {
                        base: f1,
                        exponent: 1,
                        status: PolynomialFactorStatus::ProvenIrreducible,
                    });
                    factors.push(PolynomialFactorComponent {
                        base: f2,
                        exponent: 1,
                        status: PolynomialFactorStatus::ProvenIrreducible,
                    });
                    return Ok(PolynomialFactorization {
                        ring,
                        unit,
                        factors,
                        cofactor: Polynomial::zero(ring),
                        cofactor_status: PolynomialCofactorStatus::One,
                        input_rejected: false,
                        resource_exhausted: false,
                    });
                }
                QuadraticSplit::Irreducible => {
                    factors.push(PolynomialFactorComponent {
                        base: poly,
                        exponent: 1,
                        status: PolynomialFactorStatus::ProvenIrreducible,
                    });
                    return Ok(PolynomialFactorization {
                        ring,
                        unit,
                        factors,
                        cofactor: Polynomial::zero(ring),
                        cofactor_status: PolynomialCofactorStatus::One,
                        input_rejected: false,
                        resource_exhausted: false,
                    });
                }
            }
        }

        return Ok(PolynomialFactorization {
            ring,
            unit,
            factors,
            cofactor: poly,
            cofactor_status: PolynomialCofactorStatus::Unsplit,
            input_rejected: false,
            resource_exhausted: false,
        });
    }
}

enum QuadraticSplit {
    TwoLinears(Polynomial, Polynomial),
    Irreducible,
}

fn split_quadratic_over_rationals(poly: &Polynomial, rings: &RingTable) -> Result<QuadraticSplit> {
    let (a, b, c) = quadratic_coeffs(poly)?;
    // disc = b² - 4ac
    let b2 = num_mul(clone_number(&b), clone_number(&b))?;
    let four_ac = num_mul(num_mul(Number::small_int(4), clone_number(&a))?, clone_number(&c))?;
    let disc = num_add(b2, num_neg(four_ac))?;
    let Some(disc_r) = number_as_rational(&disc) else {
        return Ok(QuadraticSplit::Irreducible);
    };
    if disc_r.is_negative() {
        return Ok(QuadraticSplit::Irreducible);
    }
    let num = disc_r.numerator();
    let den = disc_r.denominator();
    let Some(sn) = isqrt_if_exact(&num) else {
        return Ok(QuadraticSplit::Irreducible);
    };
    let Some(sd) = isqrt_if_exact(&den) else {
        return Ok(QuadraticSplit::Irreducible);
    };
    let sqrt_disc = Number::from_rational_normalized(Rational::new(sn, sd));
    // roots (-b ± √d) / (2a)
    let two_a = num_mul(Number::small_int(2), clone_number(&a))?;
    let neg_b = num_neg(clone_number(&b));
    let r1 = num_div(num_add(clone_number(&neg_b), clone_number(&sqrt_disc))?, clone_number(&two_a))?;
    let r2 = num_div(num_add(neg_b, num_neg(sqrt_disc))?, two_a)?;
    let ring = poly.ring();
    let f1 = linear_factor_for_root(ring, rings, &r1)?;
    let f2 = linear_factor_for_root(ring, rings, &r2)?;
    Ok(QuadraticSplit::TwoLinears(f1, f2))
}

fn quadratic_coeffs(poly: &Polynomial) -> Result<(Number, Number, Number)> {
    let mut a = Number::small_int(0);
    let mut b = Number::small_int(0);
    let mut c = Number::small_int(0);
    for term in poly.terms() {
        let exps = term.exponents();
        if exps.len() != 1 {
            return Err(diag_poly("quadratic_multivariate"));
        }
        match exps[0] {
            2 => a = clone_number(term.coefficient()),
            1 => b = clone_number(term.coefficient()),
            0 => c = clone_number(term.coefficient()),
            _ => return Err(diag_poly("quadratic_unexpected_degree")),
        }
    }
    Ok((a, b, c))
}

fn find_rational_root(poly: &Polynomial) -> Result<Option<Number>> {
    let mut constant = Number::small_int(0);
    let mut leading = Number::small_int(0);
    let mut max_deg = 0u32;
    for term in poly.terms() {
        let d = term.exponents().first().copied().unwrap_or(0);
        if d == 0 {
            constant = clone_number(term.coefficient());
        }
        if d >= max_deg {
            max_deg = d;
            leading = clone_number(term.coefficient());
        }
    }
    let Some(c_r) = number_as_rational(&constant) else {
        return Ok(None);
    };
    let Some(a_r) = number_as_rational(&leading) else {
        return Ok(None);
    };
    // Clear denominators: work with integer constant/leading of content-cleared poly.
    let c_int = c_r.numerator();
    let a_int = a_r.numerator();
    for p in signed_divisors(&c_int) {
        for q in positive_divisors(&a_int) {
            if q.is_zero() {
                continue;
            }
            let cand = Number::from_rational_normalized(Rational::new(clone_integer(&p), clone_integer(&q)));
            if eval_poly_at(poly, &cand)?.is_zero() {
                return Ok(Some(cand));
            }
        }
    }
    Ok(None)
}

fn eval_poly_at(poly: &Polynomial, x: &Number) -> Result<Number> {
    let mut acc = Number::small_int(0);
    for term in poly.terms() {
        let deg = term.exponents().first().copied().unwrap_or(0);
        let mut pow = Number::small_int(1);
        for _ in 0..deg {
            pow = num_mul(pow, clone_number(x))?;
        }
        let monomial = num_mul(clone_number(term.coefficient()), pow)?;
        acc = num_add(acc, monomial)?;
    }
    Ok(acc)
}

fn linear_factor_for_root(ring: RingId, rings: &RingTable, root: &Number) -> Result<Polynomial> {
    // q x - p for root p/q
    let Some(r) = number_as_rational(root) else {
        return Err(diag_poly("root_not_rational"));
    };
    let p = r.numerator();
    let q = r.denominator();
    let mut b = PolynomialBuilder::new(ring);
    b.push_term(Number::from_rational_normalized(Rational::new(clone_integer(&q), Integer::one())), vec![1])?;
    b.push_term(
        Number::from_rational_normalized(Rational::new(p.neg(), Integer::one())),
        vec![0],
    )?;
    b.build(rings)
}

fn number_as_rational(n: &Number) -> Option<Rational> {
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(clone_integer(i), Integer::one()));
    }
    n.as_rational().map(clone_rational)
}

fn positive_divisors(n: &Integer) -> Vec<Integer> {
    let abs = n.abs();
    if abs.is_zero() {
        return vec![Integer::one()];
    }
    if let Some(v) = abs.to_i64() {
        let mut out = Vec::new();
        let mut i = 1i64;
        while i.saturating_mul(i) <= v {
            if v % i == 0 {
                out.push(Integer::from_i64(i));
                let other = v / i;
                if other != i {
                    out.push(Integer::from_i64(other));
                }
            }
            i += 1;
        }
        out.sort_by(|a, b| a.cmp(b));
        out
    }
    else {
        vec![Integer::one(), abs]
    }
}

fn signed_divisors(n: &Integer) -> Vec<Integer> {
    let mut out = Vec::new();
    for d in positive_divisors(n) {
        out.push(d.neg());
        out.push(d);
    }
    if out.is_empty() {
        out.push(Integer::from_i64(-1));
        out.push(Integer::one());
    }
    out
}

fn diag_poly(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::DomainError)
        .detail("domain", "polynomial")
        .detail("operation", "factor_univariate")
        .detail("reason", reason)
}

fn total_degree_univariate(poly: &Polynomial) -> Result<u32> {
    let mut max = 0u32;
    for term in poly.terms() {
        let term_deg: u32 = term.exponents().iter().sum();
        max = max.max(term_deg);
        if term.exponents().iter().filter(|&&e| e != 0).count() > 1 {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "factor_requires_univariate"));
        }
    }
    Ok(max)
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
