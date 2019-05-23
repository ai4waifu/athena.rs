//! 从 AthenaIR 关系根组装 [`SolveProblem`]。

use athena_ir::TermStore;
use athena_types::{AssumptionSetId, Diagnostic, TermId};

use super::{
    binding::BoundSymbol,
    domain::SolveDomain,
    goal::SolveGoal,
    normalize::{RelationalOperators, normalize_constraint_conjunction},
    policy::{ExecutionLimits, SolvePolicy},
    problem::SolveProblem,
};

/// 由 IR 方程/不等式根归一化并组装 [`SolveProblem`]。
///
/// - 使用 [`normalize_constraint_conjunction`]，保留关系方向与 span
/// - `goal` 必须由 lowering / 调用方显式给出，禁止从字符串推断
/// - 不修改 [`crate::reasoning::solver::SolverRequest`]
pub fn assemble_solve_problem(
    arena: &TermStore,
    equation_roots: &[TermId],
    ops: &RelationalOperators,
    unknowns: Vec<BoundSymbol>,
    parameters: Vec<BoundSymbol>,
    domain: SolveDomain,
    assumptions: AssumptionSetId,
    goal: SolveGoal,
    policy: SolvePolicy,
    limits: ExecutionLimits,
) -> Result<SolveProblem, Diagnostic> {
    let constraints = normalize_constraint_conjunction(arena, equation_roots, ops)?;
    SolveProblem::try_new(constraints, unknowns, parameters, domain, assumptions, goal, policy, limits)
}
