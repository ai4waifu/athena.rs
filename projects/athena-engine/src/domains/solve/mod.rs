//! Solve 数学合同：`Constraint` / `SolveProblem` / `SolutionSet`。
//!
//! 与 [`crate::reasoning::solver`] 调度协议分离：
//! - 本模块拥有跨域求解的数学对象与覆盖语义
//! - `solver/` 只负责 Reflector / Registry / Frontier / [`crate::reasoning::solver::SolverRequest`]
//!
//! 禁止新增 `athena-solver` crate，也禁止把 [`crate::reasoning::solver::SolverRequest`] 扩展成
//! [`SolveProblem`]。

mod adapters_linear;
mod adapters_univariate;
mod binding;
mod certificate;
mod constraint;
mod coverage;
mod dispatch;
mod domain;
mod frontier;
mod goal;
mod map_coverage;
mod normalize;
mod policy;
mod problem;
mod problem_from_ir;
mod relation;
mod solution;
mod value_table;

pub use adapters_linear::{
    LinearAdaptedSolution, adapt_exact_linear_solve, adapt_machine_linear_solve, solve_linear_system_exact,
    solve_linear_system_machine,
};
pub use adapters_univariate::{UnivariateAdaptedSolution, adapt_univariate_factorization, solve_univariate_polynomial_roots};
pub use binding::{BindingId, BindingMap, BoundSymbol};
pub use certificate::{ResidualCertificate, proof_ref_from_witness};
pub use constraint::{
    Constraint, ConstraintConnective, ConstraintSet, Equation, Inequality, InequalityOp, QuantifiedConstraint, Quantifier,
    SolvePredicate,
};
pub use coverage::CoverageStatus;
pub use dispatch::{LinearSolveMode, execute_linear_system_goal, execute_polynomial_root_goal, require_goal};
pub use domain::SolveDomain;
pub use frontier::ResumeToken;
pub use goal::SolveGoal;
pub use map_coverage::{coverage_from_exact_disposition, coverage_from_factorization, coverage_from_machine_disposition};
pub use normalize::{RelationalOperators, normalize_constraint_conjunction, normalize_relational_application};
pub use policy::{ExecutionLimits, SolvePolicy};
pub use problem::SolveProblem;
pub use problem_from_ir::assemble_solve_problem;
pub use relation::SolveRelationKind;
pub use solution::{BranchStatus, MultiplicityInfo, SolutionBranch, SolutionSet};
pub use value_table::{BindingValue, BindingValueTable};
