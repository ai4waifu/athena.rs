//! Unverified equivalence candidates — never M-Graph facts.

use athena_rewriter::{LocalRewriteWitness, RewriteRuleId};
use athena_types::TermId;

use super::ids::EClassId;

/// One candidate equality produced by local saturation.
///
/// Must pass Verifier + [`crate::reasoning::mgraph::AdmissionGate`] before becoming
/// an admitted relation. Holding this value does **not** change Session / M-Graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateEquivalence {
    /// Left term root in the host [`athena_ir::TermStore`].
    pub left_term: TermId,
    /// Right term root in the host TermStore.
    pub right_term: TermId,
    /// Local e-class of the left term (diagnostic / extract).
    pub left_class: EClassId,
    /// Local e-class of the right term.
    pub right_class: EClassId,
    /// Rule that produced this candidate (`None` if not rule-driven).
    pub rule: Option<RewriteRuleId>,
}

impl CandidateEquivalence {
    /// Local rewrite witness when this candidate came from a [`RewriteRuleId`].
    pub fn local_witness(&self) -> Option<LocalRewriteWitness> {
        Some(LocalRewriteWitness { rule: self.rule?, subject: self.left_term, produced: self.right_term })
    }
}
