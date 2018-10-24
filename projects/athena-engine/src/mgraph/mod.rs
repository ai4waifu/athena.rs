//! M-Graph 状态与闭包（骨架）。
//!
//! 详见执行架构：等价类 · determinacy · hyper-edge · witness · frontier。

mod closure;
mod polynomial;
mod state;
mod types;

pub use closure::{ClosureLimits, ClosureResult, run_closure_step};
pub use polynomial::{PolynomialCacheEntry, PolynomialMGraphStore, PolynomialWitness, POLYNOMIAL_SOLVER_ID, witness_from_exact};
pub use state::MGraphState;
pub use types::{
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge, RewriteWitness,
    SolverCandidate, SolverFrontier, SolverId, SolverScore,
};
