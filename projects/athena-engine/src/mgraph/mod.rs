//! M-Graph 状态与闭包（骨架）。
//!
//! 详见执行架构：等价类 · determinacy · hyper-edge · witness · frontier。

mod closure;
mod state;
mod types;

pub use closure::{ClosureLimits, ClosureResult, run_closure_step};
pub use state::MGraphState;
pub use types::{
    DeterminacyGuarantee, DeterminacyState, EqualityWitness, EquivalenceClasses, ExactnessLevel, HyperEdge, RewriteWitness,
    SolverCandidate, SolverFrontier, SolverId, SolverScore,
};
