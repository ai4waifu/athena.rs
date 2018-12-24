//! Solve 数学合同：`Constraint` / `SolveProblem` / `SolutionSet`。
//!
//! 与 [`crate::solver`] 调度协议分离：
//! - 本模块拥有跨域求解的数学对象与覆盖语义
//! - `solver/` 只负责 Reflector / Registry / Frontier / [`crate::solver::SolverRequest`]
//!
//! 禁止新增 `athena-solver` crate，也禁止把 [`crate::solver::SolverRequest`] 扩展成
//! [`SolveProblem`]。

mod binding;
mod certificate;
mod constraint;
mod coverage;
mod domain;
mod frontier;
mod goal;
mod policy;
mod problem;
mod relation;
mod solution;

pub use binding::{BindingId, BindingMap, BoundSymbol};
pub use certificate::{ProofRef, ResidualCertificate};
pub use constraint::{
    Constraint, ConstraintConnective, ConstraintSet, Equation, Inequality, InequalityOp, QuantifiedConstraint, Quantifier,
    SolvePredicate,
};
pub use coverage::CoverageStatus;
pub use domain::SolveDomain;
pub use frontier::ResumeToken;
pub use goal::SolveGoal;
pub use policy::{ExecutionLimits, SolvePolicy};
pub use problem::SolveProblem;
pub use relation::SolveRelationKind;
pub use solution::{BranchStatus, MultiplicityInfo, SolutionBranch, SolutionSet};
