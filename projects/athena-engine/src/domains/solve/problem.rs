//! `SolveProblem` 与结构校验。

use athena_types::{AssumptionSetId, Diagnostic, DiagnosticCode};

use super::{
    binding::BoundSymbol,
    constraint::ConstraintSet,
    domain::SolveDomain,
    goal::SolveGoal,
    policy::{ExecutionLimits, SolvePolicy},
};

/// 统一求解问题对象。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct SolveProblem {
    /// 约束。
    pub constraints: ConstraintSet,
    /// 未知量（与 parameters 分离）。
    pub unknowns: Vec<BoundSymbol>,
    /// 参数。
    pub parameters: Vec<BoundSymbol>,
    /// 定义域。
    pub domain: SolveDomain,
    /// 假设集引用。
    pub assumptions: AssumptionSetId,
    /// 求解目标。
    pub goal: SolveGoal,
    /// 策略。
    pub policy: SolvePolicy,
    /// 资源限制。
    pub limits: ExecutionLimits,
}

impl SolveProblem {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            constraints: self.constraints.owning_copy(),
            unknowns: self.unknowns.clone(),
            parameters: self.parameters.clone(),
            domain: self.domain,
            assumptions: self.assumptions,
            goal: self.goal,
            policy: self.policy.owning_copy(),
            limits: self.limits,
        }
    }

    /// 构造并校验基本不变量。
    pub fn try_new(
        constraints: ConstraintSet,
        unknowns: Vec<BoundSymbol>,
        parameters: Vec<BoundSymbol>,
        domain: SolveDomain,
        assumptions: AssumptionSetId,
        goal: SolveGoal,
        policy: SolvePolicy,
        limits: ExecutionLimits,
    ) -> Result<Self, Diagnostic> {
        let problem = Self { constraints, unknowns, parameters, domain, assumptions, goal, policy, limits };
        problem.validate()?;
        Ok(problem)
    }

    /// 校验 unknowns/parameters 分离与 goal 最低要求。
    pub fn validate(&self) -> Result<(), Diagnostic> {
        if has_duplicate_symbols(&self.unknowns) {
            return Err(diag("duplicate_unknowns"));
        }
        if has_duplicate_symbols(&self.parameters) {
            return Err(diag("duplicate_parameters"));
        }
        for u in &self.unknowns {
            if self.parameters.iter().any(|p| p == u) {
                return Err(diag("unknown_parameter_overlap"));
            }
        }
        if self.unknowns.is_empty()
            && matches!(
                self.goal,
                SolveGoal::ExactSolutionSet
                    | SolveGoal::NumericalRootSet
                    | SolveGoal::LinearSystemSolve
                    | SolveGoal::PolynomialRootSet
                    | SolveGoal::LocalNumericalRoot
                    | SolveGoal::ModelFinding
                    | SolveGoal::DifferentialSolution
                    | SolveGoal::RecurrenceSolution
            )
        {
            return Err(diag("empty_unknowns_for_goal"));
        }
        Ok(())
    }
}

fn has_duplicate_symbols(symbols: &[BoundSymbol]) -> bool {
    let mut seen = symbols.to_vec();
    seen.sort_unstable();
    seen.windows(2).any(|w| w[0] == w[1])
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::TypeMismatch).detail("domain", "solve").detail("operation", "validate_problem").detail("reason", reason)
}
