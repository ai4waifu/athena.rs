//! 在 M-Graph 接纳前，回放验证带类型的重写候选。

use athena_ir::TermStore;
use athena_rewriter::{PatternBindings, match_pattern, substitute};

use crate::reasoning::{
    egraph::{CandidateEquivalence, TypedRuleSet},
    mgraph::{
        SemanticCore,
        admission::{AdmissionGate, AdmissionRejectReason, VerificationPolicy},
        facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope},
    },
};

use super::pipeline::EGRAPH_PROVIDER_ID;

/// 回放 `match_pattern` + `substitute`；当候选右侧匹配时升级为 `ProvenExact`。
pub fn verify_typed_rewrite_candidate(
    store: &mut TermStore,
    rules: &TypedRuleSet,
    candidate: &CandidateEquivalence,
) -> Result<Claim, AdmissionRejectReason> {
    let rule_id = candidate.rule.ok_or(AdmissionRejectReason::NotExact)?;
    let rule = rules.get(rule_id).ok_or(AdmissionRejectReason::NotExact)?;
    let mut binds = PatternBindings::new();
    if !match_pattern(store, candidate.left_term, &rule.pattern, &mut binds) {
        return Err(AdmissionRejectReason::NotExact);
    }
    let produced = substitute(store, rule.replacement, &binds);
    if !store.structural_eq(produced, candidate.right_term) {
        return Err(AdmissionRejectReason::NotExact);
    }
    let left = candidate.left_term;
    let right = candidate.right_term;
    Ok(Claim {
        proposition: Proposition::TermEquality { left, right },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: EGRAPH_PROVIDER_ID,
            certificate: EvidenceCertificate::TypedRewriteReplay { rule: rule_id, left, right },
            summary: format!("typed-rewrite-replay:{rule_id:?}:{left:?}:{right:?}"),
        },
    })
}

/// 验证并接纳一条带类型的重写候选。
pub fn admit_typed_rewrite_candidate(
    store: &mut TermStore,
    semantic: &mut SemanticCore,
    rules: &TypedRuleSet,
    candidate: &CandidateEquivalence,
    policy: &VerificationPolicy,
) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
    let claim = verify_typed_rewrite_candidate(store, rules, candidate)?;
    AdmissionGate::admit_claim(semantic, claim, policy)
}

/// 接纳在 `rules` 下回放成功的全部候选（跳过非规则 / 回放失败者）。
pub fn admit_typed_rewrite_candidates(
    store: &mut TermStore,
    semantic: &mut SemanticCore,
    rules: &TypedRuleSet,
    candidates: &[CandidateEquivalence],
    policy: &VerificationPolicy,
) -> Vec<Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason>> {
    candidates.iter().filter(|c| c.rule.is_some()).map(|c| admit_typed_rewrite_candidate(store, semantic, rules, c, policy)).collect()
}
