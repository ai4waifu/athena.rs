//! Scope-local E-Graph candidate search (Living `03` / `26` / `29`).
//!
//! ```text
//! TermStore (immutable semantics)
//!     ↓ add / hash-cons enodes
//! E-Graph (budgeted local equivalence candidates)
//!     ↓ CandidateEquivalence
//! Verifier → AdmissionGate → M-Graph (authority)
//! ```
//!
//! Hard rules:
//! - Never admit facts into M-Graph from this module.
//! - Never unbounded equality saturation.
//! - Exact / conditional / approximate must not share one union-find with M-Graph
//!   [`ExactUnionFind`](crate::reasoning::mgraph::ExactUnionFind).

mod budget;
mod candidate;
mod congruence;
mod extract;
mod graph;
mod ids;
mod pipeline;
mod saturation;
mod typed_rules;

pub use budget::{SaturationBudget, SaturationStopReason};
pub use candidate::CandidateEquivalence;
pub use congruence::{
    admit_application_congruence, admit_application_congruence_candidates, application_congruence_candidates,
    applications_congruent, verify_application_congruence,
};
pub use extract::{ExtractionPreference, Extractor, ResultCost};
pub use graph::EGraph;
pub use ids::{EClassId, ENodeId};
pub use pipeline::{
    EGRAPH_PROVIDER_ID, admit_structural_candidates, admit_structural_term_equality, candidate_to_outer,
    verify_structural_term_equality,
};
pub use saturation::{SaturationReport, saturate, saturate_typed};
pub use typed_rules::{TypedRewriteRule, TypedRuleSet};

#[cfg(test)]
mod tests;
