//! Evidence admission gate — 唯一可信接纳边界。
//!
//! 外部 solver / 缓存命中均不可直接写入 semantic core；须通过本模块。

use super::claim::{Claim, Evidence, Guarantee, Scope, VerifiedClaim, proposition_from_cache_key};
use super::polynomial::{PolynomialWitness, POLYNOMIAL_SOLVER_ID, witness_from_exact};
use crate::polynomial::{PolynomialCacheKey, PolynomialDomainValue, PolynomialResult};

/// 拒绝接纳原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRejectReason {
    /// 占位结果。
    Placeholder,
    /// Gröbner 在资源限制内未完成。
    GroebnerIncomplete,
    /// 结果枚举非 Exact。
    NotExact,
    /// 保证层级不足以进入 exact closure。
    InsufficientGuarantee,
}

/// Admission 判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// 已验证并接纳。
    Admitted(VerifiedClaim),
    /// 拒绝（可缓存，不可进 semantic core）。
    Rejected {
        /// 原因。
        reason: AdmissionRejectReason,
        /// 对应的非 exact 保证（用于审计）。
        guarantee: Guarantee,
    },
}

/// 对多项式 Exact 结果执行 admission 检查。
pub fn admit_polynomial_exact(key: &PolynomialCacheKey, value: &PolynomialDomainValue) -> AdmissionOutcome {
    let guarantee = classify_polynomial_guarantee(value);
    if !guarantee_admits_to_semantic_core(guarantee) {
        return AdmissionOutcome::Rejected {
            reason: reject_reason_for_value(value, guarantee),
            guarantee,
        };
    }
    let witness = witness_from_exact(key, value);
    let claim = Claim {
        proposition: proposition_from_cache_key(key),
        scope: Scope::Unconditional,
        guarantee,
        evidence: evidence_from_witness(&witness),
    };
    AdmissionOutcome::Admitted(VerifiedClaim::new(claim))
}

/// 对 [`PolynomialResult`] 执行 admission（Exact 分支）。
pub fn admit_polynomial_result(key: &PolynomialCacheKey, result: &PolynomialResult) -> AdmissionOutcome {
    match result {
        PolynomialResult::Exact { value } => admit_polynomial_exact(key, value),
        PolynomialResult::Unevaluated { .. } => AdmissionOutcome::Rejected {
            reason: AdmissionRejectReason::NotExact,
            guarantee: Guarantee::Unknown,
        },
    }
}

/// 是否应写入 M-Graph verified fact log。
pub fn is_admitted(outcome: &AdmissionOutcome) -> bool {
    matches!(outcome, AdmissionOutcome::Admitted(_))
}

fn classify_polynomial_guarantee(value: &PolynomialDomainValue) -> Guarantee {
    match value {
        PolynomialDomainValue::Polynomial(_) => Guarantee::ProvenExact,
        PolynomialDomainValue::GroebnerBasis(v) => {
            if v.certificate.complete {
                Guarantee::ProvenExact
            } else {
                Guarantee::Partial
            }
        }
        PolynomialDomainValue::Placeholder => Guarantee::Unknown,
    }
}

fn guarantee_admits_to_semantic_core(guarantee: Guarantee) -> bool {
    matches!(guarantee, Guarantee::ProvenExact)
}

fn reject_reason_for_value(value: &PolynomialDomainValue, guarantee: Guarantee) -> AdmissionRejectReason {
    match value {
        PolynomialDomainValue::Placeholder => AdmissionRejectReason::Placeholder,
        PolynomialDomainValue::GroebnerBasis(_) if guarantee == Guarantee::Partial => {
            AdmissionRejectReason::GroebnerIncomplete
        }
        _ => AdmissionRejectReason::InsufficientGuarantee,
    }
}

fn evidence_from_witness(witness: &PolynomialWitness) -> Evidence {
    Evidence::TrustedKernel {
        solver: POLYNOMIAL_SOLVER_ID,
        summary: format!("{}:{}", witness.operation.as_str(), witness.output_summary),
    }
}
