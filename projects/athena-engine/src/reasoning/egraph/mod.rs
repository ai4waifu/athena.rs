//! 作用域局部 E-Graph 候选搜索。
//!
//! ```text
//! TermStore（不可变语义）
//!     ↓ 添加 / hash-cons enodes
//! E-Graph（带预算的局部等价候选）
//!     ↓ CandidateEquivalence
//! Verifier → AdmissionGate → M-Graph（权威）
//! ```
//!
//! 硬性规则：
//! - 本模块绝不可向 M-Graph 接纳事实。
//! - 绝不可做无界等价饱和。
//! - Exact / conditional / approximate 不得与 M-Graph 的
//!   [`ExactUnionFind`](crate::reasoning::mgraph::ExactUnionFind) 共用一个并查集。

mod budget;
mod candidate;
mod congruence;
mod extract;
mod graph;
mod ids;
mod pipeline;
mod rewrite_verify;
mod saturation;
mod typed_rules;

pub use budget::{SaturationBudget, SaturationStopReason};
pub use candidate::CandidateEquivalence;
pub use congruence::{
    admit_application_congruence, admit_application_congruence_candidates, application_congruence_candidates, applications_congruent,
    verify_application_congruence,
};
pub use extract::{ExtractionPreference, Extractor, ParetoFrontier, ResultCost};
pub use graph::EGraph;
pub use ids::{EClassId, ENodeId};
pub use pipeline::{
    EGRAPH_PROVIDER_ID, admit_structural_candidates, admit_structural_term_equality, candidate_to_outer, verify_structural_term_equality,
};
pub use rewrite_verify::{admit_typed_rewrite_candidate, admit_typed_rewrite_candidates, verify_typed_rewrite_candidate};
pub use saturation::{SaturationReport, saturate, saturate_typed};
pub use typed_rules::{TypedRewriteRule, TypedRuleSet};
