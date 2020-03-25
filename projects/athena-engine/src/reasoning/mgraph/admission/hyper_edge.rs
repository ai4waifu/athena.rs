//! 将带类型的 [`HyperEdge`] 暂存为未验证的 [`OuterCandidate`]。

use athena_ir::{TermStore, canonical_hash};
use athena_types::TermId;

use crate::{
    domains::polynomial::PolynomialCacheOp,
    reasoning::mgraph::{
        admission::{AdmissionRejectReason, OuterCandidate},
        core::{
            predicate_registry::{arity_ok, descriptor},
            predicates,
            types::{CapabilityProviderId, HyperEdge},
        },
        facts::claim::{CalculusRelationKind, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope},
    },
};

/// 暂存超边候选的能力提供者标识（非受信任内核）。
pub const HYPER_EDGE_STAGING_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(22);

/// 将带类型超边转为未验证的外层候选。
///
/// **不会** 接纳。可暂存的谓词：
/// - [`predicates::REWRITE_EQUIVALENT`] / [`predicates::EVALUATION_RESULT`] → `TermEquality`
/// - [`predicates::DERIVATIVE_OF`] / [`predicates::INTEGRAL_OF`] / [`predicates::SERIES_EXPANSION`]
///   → `CalculusRelation`（表达式/变量经 [`canonical_hash`]，结果为 `TermId`）
/// - [`predicates::CONGRUENCE`] → `Congruence`（左/右/模的指纹）
/// - [`predicates::POLYNOMIAL_RESULT`] → `PolynomialResult`（请求指纹；暂存操作
///   [`PolynomialCacheOp::Normalize`]）
///
/// 其余谓词在命题映射就绪前仍留在 solver/reflector 局部。
pub fn hyper_edge_to_outer_candidate(store: &TermStore, edge: &HyperEdge) -> Result<OuterCandidate, AdmissionRejectReason> {
    if descriptor(edge.predicate).is_none() || !arity_ok(edge.predicate, edge.nodes.len()) {
        return Err(AdmissionRejectReason::MalformedRelation);
    }
    if edge.predicate == predicates::REWRITE_EQUIVALENT || edge.predicate == predicates::EVALUATION_RESULT {
        let [left, right] = binary_terms(&edge.nodes)?;
        require_present(store, left)?;
        require_present(store, right)?;
        let tag = if edge.predicate == predicates::REWRITE_EQUIVALENT { "hyper-edge-rewrite" } else { "hyper-edge-eval" };
        return Ok(candidate_claim(Proposition::TermEquality { left, right }, format!("{tag}:{left:?}:{right:?}")));
    }
    if let Some(kind) = calculus_kind(edge.predicate) {
        let [expr, var, result] = ternary_terms(&edge.nodes)?;
        let expression_fingerprint = term_fingerprint(store, expr)?;
        let variable_fingerprint = term_fingerprint(store, var)?;
        require_present(store, result)?;
        return Ok(candidate_claim(
            Proposition::CalculusRelation { kind, expression_fingerprint, variable_fingerprint, result_term: result },
            format!("hyper-edge-calculus:{kind:?}:{expr:?}:{var:?}:{result:?}"),
        ));
    }
    if edge.predicate == predicates::CONGRUENCE {
        let [left, right, modulus] = ternary_terms(&edge.nodes)?;
        return Ok(candidate_claim(
            Proposition::Congruence {
                left: term_fingerprint(store, left)?,
                right: term_fingerprint(store, right)?,
                modulus_fingerprint: term_fingerprint(store, modulus)?,
            },
            format!("hyper-edge-congruence:{left:?}:{right:?}:{modulus:?}"),
        ));
    }
    if edge.predicate == predicates::POLYNOMIAL_RESULT {
        let [request] = unary_term(&edge.nodes)?;
        let request_fingerprint = term_fingerprint(store, request)?;
        return Ok(candidate_claim(
            Proposition::PolynomialResult {
                // 暂存占位：超边仅携带请求对象标识。
                operation: PolynomialCacheOp::Normalize,
                request_fingerprint,
            },
            format!("hyper-edge-polynomial:{request:?}"),
        ));
    }
    Err(AdmissionRejectReason::NotExact)
}

fn candidate_claim(proposition: Proposition, summary: String) -> OuterCandidate {
    OuterCandidate::new(Claim {
        proposition,
        scope: Scope::Unconditional,
        guarantee: Guarantee::Candidate,
        evidence: Evidence::TrustedKernel {
            provider: HYPER_EDGE_STAGING_PROVIDER_ID,
            certificate: EvidenceCertificate::Rejected { guarantee: Guarantee::Candidate },
            summary,
        },
    })
}

fn calculus_kind(predicate: crate::reasoning::mgraph::PredicateId) -> Option<CalculusRelationKind> {
    if predicate == predicates::DERIVATIVE_OF {
        Some(CalculusRelationKind::DerivativeOf)
    }
    else if predicate == predicates::INTEGRAL_OF {
        Some(CalculusRelationKind::IntegralOf)
    }
    else if predicate == predicates::SERIES_EXPANSION {
        Some(CalculusRelationKind::SeriesExpansion)
    }
    else {
        None
    }
}

fn binary_terms(nodes: &[TermId]) -> Result<[TermId; 2], AdmissionRejectReason> {
    match nodes {
        [left, right] => Ok([*left, *right]),
        _ => Err(AdmissionRejectReason::MalformedRelation),
    }
}

fn unary_term(nodes: &[TermId]) -> Result<[TermId; 1], AdmissionRejectReason> {
    match nodes {
        [only] => Ok([*only]),
        _ => Err(AdmissionRejectReason::MalformedRelation),
    }
}

fn ternary_terms(nodes: &[TermId]) -> Result<[TermId; 3], AdmissionRejectReason> {
    match nodes {
        [a, b, c] => Ok([*a, *b, *c]),
        _ => Err(AdmissionRejectReason::MalformedRelation),
    }
}

fn require_present(store: &TermStore, id: TermId) -> Result<(), AdmissionRejectReason> {
    if store.get(id).is_none() {
        return Err(AdmissionRejectReason::MalformedRelation);
    }
    Ok(())
}

fn term_fingerprint(store: &TermStore, id: TermId) -> Result<u64, AdmissionRejectReason> {
    require_present(store, id)?;
    Ok(canonical_hash(store, id))
}
