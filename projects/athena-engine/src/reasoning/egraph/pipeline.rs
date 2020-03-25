//! 将 E-Graph 候选桥接到 M-Graph 接纳。

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

/// E-Graph 候选验证的能力提供者标识。
pub const EGRAPH_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(20);

/// 将 E-Graph 候选转为未验证的外层声明（`Guarantee::Candidate`）。
///
/// **不会** 接纳。请先调用 [`verify_structural_term_equality`]，再调用 [`AdmissionGate::admit_claim`]。
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

/// 当 `TermStore` 报告结构相等时，升级项等式声明。
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

/// 验证结构相等后接纳进语义核心（写入 ExactUF + ProofForest）。
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

/// 仅接纳在 `store` 中结构相等的那些候选。
///
/// 会改变结构的规则驱动重写仍保持为外层候选，并被跳过。
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
