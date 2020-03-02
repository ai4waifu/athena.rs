//! 按 [`SolveGoal`] 分派到已有领域 adapter（不扩展 `SolverRequest`）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::domains::{
    linear_algebra::MatrixValue,
    polynomial::{Polynomial, PolynomialFactorLimits, RingTable},
};

use super::{
    adapters_linear::{LinearAdaptedSolution, solve_linear_system_exact, solve_linear_system_machine},
    adapters_univariate::{UnivariateAdaptedSolution, solve_univariate_polynomial_roots},
    goal::SolveGoal,
    problem::SolveProblem,
};

/// 精确 / 机器线性路径。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinearSolveMode {
    /// 精确域（有理/整数 parent）。
    Exact,
    /// 机器浮点，带主元阈值。
    Machine {
        /// 主元阈值。
        pivot_threshold: f64,
    },
}

/// 校验 problem.goal 后执行线性系统 goal。
pub fn execute_linear_system_goal(
    problem: &SolveProblem,
    a: &MatrixValue,
    b: &MatrixValue,
    mode: LinearSolveMode,
) -> Result<LinearAdaptedSolution, Diagnostic> {
    require_goal(problem, SolveGoal::LinearSystemSolve)?;
    match mode {
        LinearSolveMode::Exact => solve_linear_system_exact(a, b, problem.unknowns.clone(), problem.domain.clone()),
        LinearSolveMode::Machine { pivot_threshold } => {
            solve_linear_system_machine(a, b, problem.unknowns.clone(), problem.domain.clone(), pivot_threshold)
        }
    }
}

/// 线性系统 goal，并将可选 `ResumeToken` 登记到 `Session.frontiers`。
pub fn execute_linear_system_goal_with_session(
    session: &mut crate::runtime::Session,
    problem: &SolveProblem,
    a: &MatrixValue,
    b: &MatrixValue,
    mode: LinearSolveMode,
    goal_fingerprint: u64,
) -> Result<LinearAdaptedSolution, Diagnostic> {
    let mut adapted = execute_linear_system_goal(problem, a, b, mode)?;
    adapted.register_frontier_on_session(session, goal_fingerprint);
    Ok(adapted)
}

/// 校验 problem.goal 后执行一元多项式根 goal。
pub fn execute_polynomial_root_goal(
    problem: &SolveProblem,
    polynomial: Polynomial,
    rings: &RingTable,
    limits: PolynomialFactorLimits,
) -> Result<UnivariateAdaptedSolution, Diagnostic> {
    require_goal(problem, SolveGoal::PolynomialRootSet)?;
    if problem.unknowns.len() != 1 {
        return Err(diag("univariate_expects_one_unknown"));
    }
    let unknown = problem.unknowns[0];
    solve_univariate_polynomial_roots(polynomial, rings, unknown, problem.domain.clone(), limits)
}

/// 一元根 goal，并将可选 `ResumeToken` 登记到 `Session.frontiers`。
pub fn execute_polynomial_root_goal_with_session(
    session: &mut crate::runtime::Session,
    problem: &SolveProblem,
    polynomial: Polynomial,
    rings: &RingTable,
    limits: PolynomialFactorLimits,
    goal_fingerprint: u64,
) -> Result<UnivariateAdaptedSolution, Diagnostic> {
    let mut adapted = execute_polynomial_root_goal(problem, polynomial, rings, limits)?;
    adapted.register_frontier_on_session(session, goal_fingerprint);
    Ok(adapted)
}

/// Goal 不匹配时返回结构化诊断。
pub fn require_goal(problem: &SolveProblem, expected: SolveGoal) -> Result<(), Diagnostic> {
    if problem.goal != expected {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch)
            .detail("domain", "solve")
            .detail("operation", "require_goal")
            .detail("reason", "goal_mismatch")
            .detail("expected", format!("{expected:?}"))
            .detail("actual", format!("{:?}", problem.goal)));
    }
    Ok(())
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeMismatch).detail("domain", "solve").detail("operation", "execute_goal").detail("reason", reason)
}
