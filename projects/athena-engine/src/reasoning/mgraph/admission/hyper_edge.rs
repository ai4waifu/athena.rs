//! Stage typed [`HyperEdge`] values as unverified [`OuterCandidate`]s (Living `26`).

use athena_types::TermId;

use crate::reasoning::mgraph::{
    admission::{AdmissionRejectReason, OuterCandidate},
    core::{
        predicate_registry::{arity_ok, descriptor},
        predicates,
        types::{CapabilityProviderId, HyperEdge},
    },
    facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope},
};

/// Capability provider identity for staged hyper-edge candidates (not a trusted kernel).
pub const HYPER_EDGE_STAGING_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(22);

/// Turn a typed hyper-edge into an unverified outer candidate.
///
/// Does **not** admit. Currently only [`predicates::REWRITE_EQUIVALENT`] (binary term equality)
/// is staged; other predicates remain solver/reflector-local until a proposition mapping exists.
pub fn hyper_edge_to_outer_candidate(edge: &HyperEdge) -> Result<OuterCandidate, AdmissionRejectReason> {
    if descriptor(edge.predicate).is_none() || !arity_ok(edge.predicate, edge.nodes.len()) {
        return Err(AdmissionRejectReason::MalformedRelation);
    }
    if edge.predicate == predicates::REWRITE_EQUIVALENT {
        let [left, right] = binary_terms(&edge.nodes)?;
        return Ok(OuterCandidate::new(Claim {
            proposition: Proposition::TermEquality { left, right },
            scope: Scope::Unconditional,
            guarantee: Guarantee::Candidate,
            evidence: Evidence::TrustedKernel {
                provider: HYPER_EDGE_STAGING_PROVIDER_ID,
                certificate: EvidenceCertificate::Rejected {
                    guarantee: Guarantee::Candidate,
                },
                summary: format!("hyper-edge-rewrite:{left:?}:{right:?}"),
            },
        }));
    }
    Err(AdmissionRejectReason::NotExact)
}

fn binary_terms(nodes: &[TermId]) -> Result<[TermId; 2], AdmissionRejectReason> {
    match nodes {
        [left, right] => Ok([*left, *right]),
        _ => Err(AdmissionRejectReason::MalformedRelation),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_types::TermId;

    #[test]
    fn rewrite_hyper_edge_stages_candidate_term_equality() {
        let edge = HyperEdge {
            nodes: vec![TermId(1), TermId(2)],
            predicate: predicates::REWRITE_EQUIVALENT,
        };
        let outer = hyper_edge_to_outer_candidate(&edge).expect("stage");
        assert_eq!(outer.claim.guarantee, Guarantee::Candidate);
        assert_eq!(
            outer.claim.proposition,
            Proposition::TermEquality {
                left: TermId(1),
                right: TermId(2),
            }
        );
    }

    #[test]
    fn bad_arity_is_malformed() {
        let edge = HyperEdge {
            nodes: vec![TermId(1)],
            predicate: predicates::REWRITE_EQUIVALENT,
        };
        assert_eq!(
            hyper_edge_to_outer_candidate(&edge),
            Err(AdmissionRejectReason::MalformedRelation)
        );
    }

    #[test]
    fn unsupported_predicate_is_not_exact() {
        let edge = HyperEdge {
            nodes: vec![TermId(1), TermId(2), TermId(3)],
            predicate: predicates::CONGRUENCE,
        };
        assert_eq!(
            hyper_edge_to_outer_candidate(&edge),
            Err(AdmissionRejectReason::NotExact)
        );
    }
}
