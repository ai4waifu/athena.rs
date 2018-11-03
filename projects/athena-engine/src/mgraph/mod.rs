//! M-Graph 状态与闭包（骨架）。
//!
//! Semantic core · operational · admission gate · closure frontier。

mod admission;
mod claim;
mod closure;
mod derived;
mod exact_uf;
mod fact_log;
mod operational;
mod polynomial;
mod result_cache;
mod semantic;
mod state;
mod types;

pub use admission::{
    AdmissionGate, AdmissionOutcome, AdmissionRejectReason, EvidenceVerifier, VerificationPolicy, admit_polynomial_exact,
    admit_polynomial_result, is_admitted,
};
pub use claim::{Claim, Evidence, Guarantee, Proposition, Scope, VerifiedClaim, proposition_from_cache_key};
pub use closure::{ClosureLimits, ClosureResult, run_closure_step};
pub use derived::DerivedIndexes;
pub use exact_uf::ExactUnionFind;
pub use fact_log::{FactId, FactLog};
pub use operational::OperationalState;
pub use polynomial::{
    PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, PolynomialWitness, POLYNOMIAL_SOLVER_ID,
    witness_from_exact,
};
pub use result_cache::ResultCache;
pub use semantic::SemanticCore;
pub use state::MGraphState;
pub use types::{
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge, RewriteWitness,
    SolverCandidate, SolverFrontier, SolverId, SolverScore,
};
