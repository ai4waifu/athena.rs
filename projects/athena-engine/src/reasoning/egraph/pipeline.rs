//! Bridge E-Graph candidates into M-Graph admission (Living `26` / `29`).

use athena_ir::TermStore;
use athena_types::TermId;

use crate::reasoning::{
    egraph::CandidateEquivalence,
    mgraph::{
        SemanticCore,
        admission::{AdmissionGate, AdmissionRejectReason, OuterCandidate, VerificationPolicy},
        core::types::CapabilityProviderId,
        facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope},
    },
};

/// Capability provider identity for E-Graph candidate verification.
pub const EGRAPH_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(20);

/// Turn an E-Graph candidate into an unverified outer claim (`Guarantee::Candidate`).
///
/// Does **not** admit. Call [`verify_structural_term_equality`] then [`AdmissionGate::admit_claim`].
pub fn candidate_to_outer(candidate: &CandidateEquivalence) -> OuterCandidate {
    OuterCandidate::new(Claim {
        proposition: Proposition::TermEquality { left: candidate.left_term, right: candidate.right_term },
        scope: Scope::Unconditional,
        guarantee: Guarantee::Candidate,
        evidence: Evidence::TrustedKernel {
            provider: EGRAPH_PROVIDER_ID,
            certificate: EvidenceCertificate::StructuralTermEquality { left: candidate.left_term, right: candidate.right_term },
            summary: format!("egraph-candidate:{:?}:{:?}", candidate.left_term, candidate.right_term),
        },
    })
}

/// Upgrade a term-equality claim when TermStore reports structural equality.
pub fn verify_structural_term_equality(store: &TermStore, left: TermId, right: TermId) -> Result<Claim, AdmissionRejectReason> {
    if !store.structural_eq(left, right) {
        return Err(AdmissionRejectReason::NotExact);
    }
    Ok(Claim {
        proposition: Proposition::TermEquality { left, right },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: EGRAPH_PROVIDER_ID,
            certificate: EvidenceCertificate::StructuralTermEquality { left, right },
            summary: format!("structural-eq:{left:?}:{right:?}"),
        },
    })
}

/// Verify structural equality then admit into semantic core (writes ExactUF + ProofForest).
pub fn admit_structural_term_equality(
    store: &TermStore,
    semantic: &mut SemanticCore,
    left: TermId,
    right: TermId,
    policy: &VerificationPolicy,
) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
    let claim = verify_structural_term_equality(store, left, right)?;
    AdmissionGate::admit_claim(semantic, claim, policy)
}

/// Admit only those candidates that are structurally equal in `store`.
///
/// Rule-driven rewrites that change structure stay as outer candidates and are skipped.
pub fn admit_structural_candidates(
    store: &TermStore,
    semantic: &mut SemanticCore,
    candidates: &[CandidateEquivalence],
    policy: &VerificationPolicy,
) -> Vec<Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason>> {
    candidates
        .iter()
        .filter(|c| store.structural_eq(c.left_term, c.right_term))
        .map(|c| admit_structural_term_equality(store, semantic, c.left_term, c.right_term, policy))
        .collect()
}
