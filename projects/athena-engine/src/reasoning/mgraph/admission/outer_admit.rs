//! 操作层 [`OuterCandidate`] 的受控接纳。
//!
//! 绝不原样接纳 `Guarantee::Candidate`。`TermEquality` 候选仅可在
//! [`TermStore::structural_eq`] 成立时升级，并将证据改写为
//! [`EvidenceCertificate::StructuralTermEquality`]。

use athena_ir::TermStore;

use crate::reasoning::mgraph::{
    admission::{AdmissionGate, AdmissionRejectReason, OuterCandidate, VerificationPolicy},
    core::{state::MGraphState, types::CapabilityProviderId},
    facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition},
};

/// `OuterCandidate` 结构升级的能力提供者。
pub const OUTER_STRUCTURAL_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(23);

/// 尝试接纳 `OuterCandidate` 池的报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OuterAdmitReport {
    /// 已升级并接纳进 `SemanticCore` 的候选。
    pub admitted: u32,
    /// 仍保留在外层池中的候选。
    pub retained: u32,
}

/// 接纳外层池中结构相等的 `TermEquality` 对。
///
/// 不匹配 / 非 `TermEquality` 的候选留在 `outer_candidates`。
/// **不会** 为重写形态的不等式伪造证明。
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
