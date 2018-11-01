//! M-Graph 状态与闭包（骨架）。
//!
//! 详见执行架构：等价类 · determinacy · hyper-edge · witness · frontier。

mod admission;
mod claim;
mod closure;
mod polynomial;
mod state;
mod types;

pub use admission::{
    AdmissionOutcome, AdmissionRejectReason, admit_polynomial_exact, admit_polynomial_result, is_admitted,
};
pub use claim::{Claim, Evidence, Guarantee, Proposition, Scope, VerifiedClaim, proposition_from_cache_key};
pub use closure::{ClosureLimits, ClosureResult, run_closure_step};
pub use polynomial::{
    PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, PolynomialWitness, POLYNOMIAL_SOLVER_ID,
    witness_from_exact,
};
pub use state::MGraphState;
pub use types::{
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge, RewriteWitness,
    SolverCandidate, SolverFrontier, SolverId, SolverScore,
};
