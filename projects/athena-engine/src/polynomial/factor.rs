//! 多项式因式分解（骨架合同 · Living `08`）。
//!
//! 完整性模型对齐数论 [`crate::number_theory::Factorization`]：禁止裸 `Vec<Polynomial>`。
//! 当前仅处理常数与一次因式；更高次数以 `Partial` / `ResourceLimited` 显式返回。

use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use crate::numeric_clone::clone_number;
use super::{canonical::canonicalize_polynomial, expr::Polynomial, ring_table::RingTable};

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

/// 单变量因式分解（骨架）。
///
/// - 零多项式 → 域错误
/// - 常数 → `Complete`（仅 `unit`）
/// - 一次 → `Complete`（`ProvenIrreducible`）
/// - 更高次数且 `deg ≤ max_degree` → `Partial`（整式作 cofactor）
/// - `deg > max_degree` → `ResourceLimited` + `input_rejected`（输入拒绝）
pub fn factor_univariate(
    polynomial: Polynomial,
    rings: &RingTable,
    limits: PolynomialFactorLimits,
) -> Result<PolynomialFactorization> {
    let poly = canonicalize_polynomial(polynomial, rings)?;
    let ring = poly.ring();
    let _desc = rings.get(ring).ok_or_else(|| ring_unknown(ring))?;

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

    // 骨架：高次暂不分解，显式 Partial。
    Ok(PolynomialFactorization {
        ring,
        unit: Number::small_int(1),
        factors: Vec::new(),
        cofactor: poly,
        cofactor_status: PolynomialCofactorStatus::Unsplit,
        input_rejected: false,
        resource_exhausted: false,
    })
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
