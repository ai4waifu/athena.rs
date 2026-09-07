//! 在 M-Graph 接纳前，回放验证带类型的重写候选。
//!
//! `TypedRewriteReplay` 的实质检查与接纳必须经同一 [`AdmissionGate`] /
//! [`EvidenceVerifier`] 路径；禁止另开 `commit` 旁路。

use athena_ir::TermStore;
use athena_rewriter::{PatternBindings, RewriteRuleId, match_pattern, substitute};
use athena_types::TermId;

use crate::reasoning::{
    egraph::{CandidateEquivalence, TypedRuleSet},
    mgraph::{
        SemanticCore,
        admission::{AdmissionGate, AdmissionRejectReason, VerificationPolicy},
        facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope},
    },
};

use super::pipeline::EGRAPH_PROVIDER_ID;

/// 在规则表上重放 `match_pattern` + `substitute`，判断 `left → right` 是否由 `rule_id` 成立。
///
/// 供 [`crate::reasoning::mgraph::admission::gate`] 实质证据检查调用（与构造 claim 同一判据）。
pub(crate) fn typed_rewrite_holds(
    store: &mut TermStore,
    rules: &TypedRuleSet,
    rule_id: RewriteRuleId,
    left: TermId,
    right: TermId,
) -> bool {
    let Some(rule) = rules.get(rule_id)
    else {
        return false;
    };
    let mut binds = PatternBindings::new();
    if !match_pattern(store, left, &rule.pattern, &mut binds) {
        return false;
    }
    let produced = substitute(store, rule.replacement, &binds);
    store.structural_eq(produced, right)
}

/// 回放 `match_pattern` + `substitute`；当候选右侧匹配时升级为 `ProvenExact`。
pub fn verify_typed_rewrite_candidate(
    store: &mut TermStore,
    rules: &TypedRuleSet,
    candidate: &CandidateEquivalence,
) -> Result<Claim, AdmissionRejectReason> {
    let rule_id = candidate.rule.ok_or(AdmissionRejectReason::NotExact)?;
    if !typed_rewrite_holds(store, rules, rule_id, candidate.left_term, candidate.right_term) {
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

/// 验证并经统一 [`AdmissionGate`] 接纳（携带规则表供 verifier 重放）。
pub fn admit_typed_rewrite_candidate(
    store: &mut TermStore,
    semantic: &mut SemanticCore,
    rules: &TypedRuleSet,
    candidate: &CandidateEquivalence,
    policy: &VerificationPolicy,
) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
    let claim = verify_typed_rewrite_candidate(store, rules, candidate)?;
    AdmissionGate::admit_claim(store, semantic, claim, policy, Some(rules))
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
