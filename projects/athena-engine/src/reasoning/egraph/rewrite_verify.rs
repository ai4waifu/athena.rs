//! Replay-verify typed rewrite candidates before M-Graph admission (Living `03` R-2.5 / `26`).

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

/// Replay `match_pattern` + `substitute` and upgrade to ProvenExact when the candidate right-hand side matches.
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

/// Verify then admit one typed rewrite candidate.
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

/// Admit all candidates that replay successfully under `rules` (skips non-rule / failed replay).
pub fn admit_typed_rewrite_candidates(
    store: &mut TermStore,
    semantic: &mut SemanticCore,
    rules: &TypedRuleSet,
    candidates: &[CandidateEquivalence],
    policy: &VerificationPolicy,
) -> Vec<Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason>> {
    candidates.iter().filter(|c| c.rule.is_some()).map(|c| admit_typed_rewrite_candidate(store, semantic, rules, c, policy)).collect()
}
