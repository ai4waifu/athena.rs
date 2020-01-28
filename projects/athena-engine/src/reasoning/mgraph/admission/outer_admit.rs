//! Controlled admission of operational [`OuterCandidate`]s (Living `26` / `29`).
//!
//! Never admits `Guarantee::Candidate` as-is. TermEquality candidates may upgrade
//! only when [`TermStore::structural_eq`] holds, then rewrite evidence to
//! [`EvidenceCertificate::StructuralTermEquality`].

use athena_ir::TermStore;

use crate::reasoning::mgraph::{
    admission::{AdmissionGate, AdmissionRejectReason, OuterCandidate, VerificationPolicy},
    core::{state::MGraphState, types::CapabilityProviderId},
    facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition},
};

/// Capability provider for OuterCandidate structural upgrades.
pub const OUTER_STRUCTURAL_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(23);

/// Report from attempting to admit the OuterCandidate pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OuterAdmitReport {
    /// Candidates upgraded and admitted into SemanticCore.
    pub admitted: u32,
    /// Candidates retained in the outer pool.
    pub retained: u32,
}

/// Admit OuterCandidate pool entries that are structurally equal TermEquality pairs.
///
/// Non-matching / non-TermEquality candidates stay in `outer_candidates`.
/// Does **not** invent proofs for rewrite-shaped inequalities.
pub fn admit_outer_pool_if_structural(store: &TermStore, state: &mut MGraphState, policy: &VerificationPolicy) -> OuterAdmitReport {
    let pending = std::mem::take(&mut state.operational.outer_candidates);
    let mut retained = Vec::new();
    let mut admitted = 0u32;
    for outer in pending {
        match try_admit_outer_if_structural(store, &mut state.semantic, &outer, policy) {
            Ok(()) => admitted = admitted.saturating_add(1),
            Err(_) => retained.push(outer),
        }
    }
    let retained_count = retained.len() as u32;
    state.operational.outer_candidates = retained;
    OuterAdmitReport { admitted, retained: retained_count }
}

fn try_admit_outer_if_structural(
    store: &TermStore,
    semantic: &mut crate::reasoning::mgraph::SemanticCore,
    outer: &OuterCandidate,
    policy: &VerificationPolicy,
) -> Result<(), AdmissionRejectReason> {
    let Proposition::TermEquality { left, right } = outer.claim.proposition
    else {
        return Err(AdmissionRejectReason::NotExact);
    };
    if !store.structural_eq(left, right) {
        return Err(AdmissionRejectReason::NotExact);
    }
    let claim = Claim {
        proposition: Proposition::TermEquality { left, right },
        scope: outer.claim.scope,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: OUTER_STRUCTURAL_PROVIDER_ID,
            certificate: EvidenceCertificate::StructuralTermEquality { left, right },
            summary: format!("outer-structural:{left:?}:{right:?}"),
        },
    };
    AdmissionGate::admit_claim(semantic, claim, policy).map(|_| ())
}

#[cfg(test)]
mod tests {
    use athena_ir::{Atom, TermNode};
    use athena_types::SourceSpan;

    use crate::reasoning::mgraph::{HyperEdge, Scope, drain_hyper_edges_to_outer_pool, predicates};

    use super::*;

    #[test]
    fn structural_outer_pair_admits_and_clears_pool() {
        let mut store = athena_ir::TermStore::new();
        let span = SourceSpan::default();
        let a = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(7))), span);
        let b = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(7))), span);
        assert_eq!(a, b);

        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge { nodes: vec![a, b], predicate: predicates::REWRITE_EQUIVALENT });
        assert_eq!(drain_hyper_edges_to_outer_pool(&store, &mut state).staged, 1);
        assert_eq!(state.operational.outer_candidates.len(), 1);

        let report = admit_outer_pool_if_structural(&store, &mut state, &VerificationPolicy::default());
        assert_eq!(report.admitted, 1);
        assert_eq!(report.retained, 0);
        assert!(state.operational.outer_candidates.is_empty());
        assert_eq!(state.semantic.derived.exact_uf.find(a), state.semantic.derived.exact_uf.find(b));
    }

    #[test]
    fn unequal_outer_pair_stays_in_pool() {
        let mut store = athena_ir::TermStore::new();
        let span = SourceSpan::default();
        let x = store.symbols_mut().intern("x");
        let y = store.symbols_mut().intern("y");
        let left = store.push(TermNode::Atom(Atom::Symbol(x)), span);
        let right = store.push(TermNode::Atom(Atom::Symbol(y)), span);

        let mut state = MGraphState::new();
        state.operational.outer_candidates.push(OuterCandidate::new(Claim {
            proposition: Proposition::TermEquality { left, right },
            scope: Scope::Unconditional,
            guarantee: Guarantee::Candidate,
            evidence: Evidence::TrustedKernel {
                provider: OUTER_STRUCTURAL_PROVIDER_ID,
                certificate: EvidenceCertificate::Rejected { guarantee: Guarantee::Candidate },
                summary: "unequal".into(),
            },
        }));

        let report = admit_outer_pool_if_structural(&store, &mut state, &VerificationPolicy::default());
        assert_eq!(report.admitted, 0);
        assert_eq!(report.retained, 1);
        assert_eq!(state.operational.outer_candidates.len(), 1);
        assert_eq!(state.semantic.relation_count(), 0);
    }
}
