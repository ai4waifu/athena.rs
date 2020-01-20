//! Stage typed [`HyperEdge`] values as unverified [`OuterCandidate`]s (Living `26`).

use athena_ir::{TermStore, canonical_hash};
use athena_types::TermId;

use crate::reasoning::mgraph::{
    admission::{AdmissionRejectReason, OuterCandidate},
    core::{
        predicate_registry::{arity_ok, descriptor},
        predicates,
        types::{CapabilityProviderId, HyperEdge},
    },
    facts::claim::{
        CalculusRelationKind, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope,
    },
};
use crate::domains::polynomial::PolynomialCacheOp;

/// Capability provider identity for staged hyper-edge candidates (not a trusted kernel).
pub const HYPER_EDGE_STAGING_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(22);

/// Turn a typed hyper-edge into an unverified outer candidate.
///
/// Does **not** admit. Stageable predicates:
/// - [`predicates::REWRITE_EQUIVALENT`] / [`predicates::EVALUATION_RESULT`] → `TermEquality`
/// - [`predicates::DERIVATIVE_OF`] / [`predicates::INTEGRAL_OF`] / [`predicates::SERIES_EXPANSION`]
///   → `CalculusRelation` (expression/variable via [`canonical_hash`], result as `TermId`)
/// - [`predicates::CONGRUENCE`] → `Congruence` (left/right/modulus fingerprints)
/// - [`predicates::POLYNOMIAL_RESULT`] → `PolynomialResult` (request fingerprint; staging op
///   [`PolynomialCacheOp::Normalize`])
///
/// Other predicates remain solver/reflector-local until a proposition mapping exists.
pub fn hyper_edge_to_outer_candidate(
    store: &TermStore,
    edge: &HyperEdge,
) -> Result<OuterCandidate, AdmissionRejectReason> {
    if descriptor(edge.predicate).is_none() || !arity_ok(edge.predicate, edge.nodes.len()) {
        return Err(AdmissionRejectReason::MalformedRelation);
    }
    if edge.predicate == predicates::REWRITE_EQUIVALENT || edge.predicate == predicates::EVALUATION_RESULT {
        let [left, right] = binary_terms(&edge.nodes)?;
        require_present(store, left)?;
        require_present(store, right)?;
        let tag = if edge.predicate == predicates::REWRITE_EQUIVALENT {
            "hyper-edge-rewrite"
        } else {
            "hyper-edge-eval"
        };
        return Ok(candidate_claim(
            Proposition::TermEquality { left, right },
            format!("{tag}:{left:?}:{right:?}"),
        ));
    }
    if let Some(kind) = calculus_kind(edge.predicate) {
        let [expr, var, result] = ternary_terms(&edge.nodes)?;
        let expression_fingerprint = term_fingerprint(store, expr)?;
        let variable_fingerprint = term_fingerprint(store, var)?;
        require_present(store, result)?;
        return Ok(candidate_claim(
            Proposition::CalculusRelation {
                kind,
                expression_fingerprint,
                variable_fingerprint,
                result_term: result,
            },
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
                // Staging placeholder: hyper-edges carry only the request object identity.
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
            certificate: EvidenceCertificate::Rejected {
                guarantee: Guarantee::Candidate,
            },
            summary,
        },
    })
}

fn calculus_kind(predicate: crate::reasoning::mgraph::PredicateId) -> Option<CalculusRelationKind> {
    if predicate == predicates::DERIVATIVE_OF {
        Some(CalculusRelationKind::DerivativeOf)
    } else if predicate == predicates::INTEGRAL_OF {
        Some(CalculusRelationKind::IntegralOf)
    } else if predicate == predicates::SERIES_EXPANSION {
        Some(CalculusRelationKind::SeriesExpansion)
    } else {
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

#[cfg(test)]
mod tests {
    use athena_ir::{Atom, TermNode};
    use athena_types::SourceSpan;

    use super::*;

    fn store_with_symbols() -> (TermStore, TermId, TermId, TermId) {
        let mut store = TermStore::new();
        let span = SourceSpan::default();
        let a = store.symbols_mut().intern("a");
        let b = store.symbols_mut().intern("b");
        let c = store.symbols_mut().intern("c");
        let t0 = store.push(TermNode::Atom(Atom::Symbol(a)), span);
        let t1 = store.push(TermNode::Atom(Atom::Symbol(b)), span);
        let t2 = store.push(TermNode::Atom(Atom::Symbol(c)), span);
        (store, t0, t1, t2)
    }

    #[test]
    fn rewrite_hyper_edge_stages_candidate_term_equality() {
        let (store, left, right, _) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![left, right],
            predicate: predicates::REWRITE_EQUIVALENT,
        };
        let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
        assert_eq!(outer.claim.guarantee, Guarantee::Candidate);
        assert_eq!(
            outer.claim.proposition,
            Proposition::TermEquality { left, right }
        );
    }

    #[test]
    fn evaluation_result_hyper_edge_stages_term_equality() {
        let (store, left, right, _) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![left, right],
            predicate: predicates::EVALUATION_RESULT,
        };
        let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
        match &outer.claim.evidence {
            Evidence::TrustedKernel { summary, .. } => {
                assert!(summary.starts_with("hyper-edge-eval:"));
            }
        }
    }

    #[test]
    fn derivative_hyper_edge_stages_calculus_relation() {
        let (store, expr, var, result) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![expr, var, result],
            predicate: predicates::DERIVATIVE_OF,
        };
        let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
        match outer.claim.proposition {
            Proposition::CalculusRelation {
                kind,
                expression_fingerprint,
                variable_fingerprint,
                result_term,
            } => {
                assert_eq!(kind, CalculusRelationKind::DerivativeOf);
                assert_eq!(expression_fingerprint, canonical_hash(&store, expr));
                assert_eq!(variable_fingerprint, canonical_hash(&store, var));
                assert_eq!(result_term, result);
            }
            other => panic!("expected CalculusRelation, got {other:?}"),
        }
    }

    #[test]
    fn congruence_hyper_edge_stages_fingerprints() {
        let (store, left, right, modulus) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![left, right, modulus],
            predicate: predicates::CONGRUENCE,
        };
        let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
        assert_eq!(
            outer.claim.proposition,
            Proposition::Congruence {
                left: canonical_hash(&store, left),
                right: canonical_hash(&store, right),
                modulus_fingerprint: canonical_hash(&store, modulus),
            }
        );
    }

    #[test]
    fn polynomial_result_hyper_edge_stages_request_fingerprint() {
        let (store, request, _, _) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![request],
            predicate: predicates::POLYNOMIAL_RESULT,
        };
        let outer = hyper_edge_to_outer_candidate(&store, &edge).expect("stage");
        assert_eq!(
            outer.claim.proposition,
            Proposition::PolynomialResult {
                operation: PolynomialCacheOp::Normalize,
                request_fingerprint: canonical_hash(&store, request),
            }
        );
    }

    #[test]
    fn missing_term_is_malformed() {
        let store = TermStore::new();
        let edge = HyperEdge {
            nodes: vec![TermId(1), TermId(2)],
            predicate: predicates::REWRITE_EQUIVALENT,
        };
        assert_eq!(
            hyper_edge_to_outer_candidate(&store, &edge),
            Err(AdmissionRejectReason::MalformedRelation)
        );
    }

    #[test]
    fn bad_arity_is_malformed() {
        let (store, left, _, _) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![left],
            predicate: predicates::REWRITE_EQUIVALENT,
        };
        assert_eq!(
            hyper_edge_to_outer_candidate(&store, &edge),
            Err(AdmissionRejectReason::MalformedRelation)
        );
    }

    #[test]
    fn unknown_predicate_is_malformed() {
        use crate::reasoning::mgraph::PredicateId;

        let (store, only, _, _) = store_with_symbols();
        let edge = HyperEdge {
            nodes: vec![only],
            predicate: PredicateId(99),
        };
        assert_eq!(
            hyper_edge_to_outer_candidate(&store, &edge),
            Err(AdmissionRejectReason::MalformedRelation)
        );
    }
}
