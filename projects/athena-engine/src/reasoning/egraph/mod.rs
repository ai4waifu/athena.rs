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
mod extract;
mod graph;
mod ids;
mod pipeline;
mod saturation;

pub use budget::{SaturationBudget, SaturationStopReason};
pub use candidate::CandidateEquivalence;
pub use extract::{ExtractionPreference, Extractor};
pub use graph::EGraph;
pub use ids::{EClassId, ENodeId};
pub use pipeline::{
    EGRAPH_PROVIDER_ID, admit_structural_term_equality, candidate_to_outer, verify_structural_term_equality,
};
pub use saturation::{SaturationReport, saturate};

#[cfg(test)]
mod tests;
