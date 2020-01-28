//! Application congruence over ExactUF (Living `03` R-2.4 / `26`).
//!
//! When `ExactUnionFind` already equates arguments pairwise and heads match,
//! emit / admit `f(a…) ≈ f(b…)` without treating rewrite heuristics as facts.

use athena_ir::{TermNode, TermStore};
use athena_types::TermId;

use crate::reasoning::{
    egraph::{CandidateEquivalence, EGraph},
    mgraph::{
        ExactUnionFind, SemanticCore,
        admission::{AdmissionGate, AdmissionRejectReason, VerificationPolicy},
        facts::claim::{Claim, Evidence, EvidenceCertificate, Guarantee, Proposition, Scope},
    },
};

use super::pipeline::EGRAPH_PROVIDER_ID;

/// Scan known application terms and emit congruence candidates under `exact_uf`.
///
/// Does **not** admit. Pairwise scan is bounded by `max_pairs`.
pub fn application_congruence_candidates(
    store: &TermStore,
    graph: &EGraph,
    exact_uf: &ExactUnionFind,
    max_pairs: u32,
) -> Vec<CandidateEquivalence> {
    let apps: Vec<TermId> = {
        let mut terms: Vec<TermId> =
            graph.known_terms().into_iter().filter(|&t| matches!(store.get(t), Some(TermNode::Application { .. }))).collect();
        terms.sort_by_key(|t| t.0);
        terms
    };

    let mut out = Vec::new();
    let mut emitted = 0u32;
    for i in 0..apps.len() {
        for j in (i + 1)..apps.len() {
            if emitted >= max_pairs {
                return out;
            }
            let left = apps[i];
            let right = apps[j];
            if exact_uf.find(left) == exact_uf.find(right) {
                continue;
            }
            if !applications_congruent(store, exact_uf, left, right) {
                continue;
            }
            let Some(left_class) = graph.class_of_term(left)
            else {
                continue;
            };
            let Some(right_class) = graph.class_of_term(right)
            else {
                continue;
            };
            if graph.find(left_class) == graph.find(right_class) {
                continue;
            }
            out.push(CandidateEquivalence { left_term: left, right_term: right, left_class, right_class, rule: None });
            emitted = emitted.saturating_add(1);
        }
    }
    out
}

/// True when both terms are applications with equal heads and ExactUF-equal args.
pub fn applications_congruent(store: &TermStore, exact_uf: &ExactUnionFind, left: TermId, right: TermId) -> bool {
    match (store.get(left), store.get(right)) {
        (Some(TermNode::Application { head: head_l, arguments: args_l }), Some(TermNode::Application { head: head_r, arguments: args_r })) => {
            head_l == head_r
                && args_l.len() == args_r.len()
                && args_l.iter().zip(args_r.iter()).all(|(a, b)| exact_uf.find(*a) == exact_uf.find(*b))
        }
        _ => false,
    }
}

/// Build a ProvenExact claim when application congruence holds under `exact_uf`.
pub fn verify_application_congruence(
    store: &TermStore,
    exact_uf: &ExactUnionFind,
    left: TermId,
    right: TermId,
) -> Result<Claim, AdmissionRejectReason> {
    if !applications_congruent(store, exact_uf, left, right) {
        return Err(AdmissionRejectReason::NotExact);
    }
    Ok(Claim {
        proposition: Proposition::TermEquality { left, right },
        scope: Scope::Unconditional,
        guarantee: Guarantee::ProvenExact,
        evidence: Evidence::TrustedKernel {
            provider: EGRAPH_PROVIDER_ID,
            certificate: EvidenceCertificate::ApplicationCongruence { left, right },
            summary: format!("app-congruence:{left:?}:{right:?}"),
        },
    })
}

/// Admit one application-congruence equality into semantic core.
pub fn admit_application_congruence(
    store: &TermStore,
    semantic: &mut SemanticCore,
    left: TermId,
    right: TermId,
    policy: &VerificationPolicy,
) -> Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason> {
    let claim = verify_application_congruence(store, &semantic.derived.exact_uf, left, right)?;
    AdmissionGate::admit_claim(semantic, claim, policy)
}

/// Admit all application-congruence candidates that still verify under current ExactUF.
pub fn admit_application_congruence_candidates(
    store: &TermStore,
    semantic: &mut SemanticCore,
    candidates: &[CandidateEquivalence],
    policy: &VerificationPolicy,
) -> Vec<Result<crate::reasoning::mgraph::facts::FactId, AdmissionRejectReason>> {
    candidates.iter().map(|c| admit_application_congruence(store, semantic, c.left_term, c.right_term, policy)).collect()
}
