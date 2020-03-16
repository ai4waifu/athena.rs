//! 优化问题对象。

use athena_types::{AssumptionScopeId, Diagnostic, DiagnosticCode};

use super::{
    feasible::FeasibleSet,
    fingerprint::{OptimizationFingerprint, fingerprint_placeholder},
    ids::ProblemId,
    limits::OptimizationLimits,
    objective::Objective,
    variable::DecisionVariable,
};

/// 问题能力描述符（不是强制改写目标类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProblemClass {
    /// 线性规划。
    LinearProgram,
    /// 混合整数线性规划。
    MixedIntegerLinearProgram,
    /// 二次规划。
    QuadraticProgram,
    /// 二阶锥规划。
    SecondOrderConeProgram,
    /// 半正定规划。
    SemidefiniteProgram,
    /// 非线性规划。
    NonlinearProgram,
    /// 多项式优化。
    PolynomialOptimization,
    /// 一般约束优化。
    ConstraintOptimization,
    /// 多目标。
    MultiObjective,
}

/// 算法策略版本（进入 fingerprint / 缓存键）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct AlgorithmPolicy {
    /// 策略标识（如 `primal-simplex` / `bnb-v0`）。
    pub name: String,
    /// 策略版本。
    pub version: u32,
}

impl AlgorithmPolicy {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self { name: self.name.clone(), version: self.version }
    }
}

impl Default for AlgorithmPolicy {
    fn default() -> Self {
        Self { name: "unset".to_string(), version: 0 }
    }
}

/// 优化问题。
///
/// 稳定身份是 [`OptimizationFingerprint`]；[`ProblemId`] 仅 Session-local。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct OptimizationProblem {
    /// Session-local 句柄。
    pub id: ProblemId,
    /// 稳定指纹。
    pub fingerprint: OptimizationFingerprint,
    /// 能力分类。
    pub class: ProblemClass,
    /// 决策变量。
    pub variables: Vec<DecisionVariable>,
    /// 可行集。
    pub feasible_set: FeasibleSet,
    /// 目标（多目标时按 priority 排序约定由调用方保证）。
    pub objectives: Vec<Objective>,
    /// 假设作用域。
    pub assumptions: AssumptionScopeId,
    /// 算法策略。
    pub policy: AlgorithmPolicy,
    /// 资源与容差。
    pub limits: OptimizationLimits,
}

impl OptimizationProblem {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            fingerprint: self.fingerprint,
            class: self.class,
            variables: self.variables.iter().map(DecisionVariable::owning_copy).collect(),
            feasible_set: self.feasible_set.owning_copy(),
            objectives: self.objectives.clone(),
            assumptions: self.assumptions,
            policy: self.policy.owning_copy(),
            limits: self.limits,
        }
    }

    /// 构造骨架问题；拒绝整数性/域不一致。
    pub fn try_new(
        id: ProblemId,
        class: ProblemClass,
        variables: Vec<DecisionVariable>,
        feasible_set: FeasibleSet,
        objectives: Vec<Objective>,
        assumptions: AssumptionScopeId,
        policy: AlgorithmPolicy,
        limits: OptimizationLimits,
    ) -> Result<Self, Diagnostic> {
        for v in &variables {
            if !v.integrality_consistent() {
                return Err(Diagnostic::new(DiagnosticCode::DomainError)
                    .detail("domain", "optimization")
                    .detail("reason", "integrality_domain_mismatch")
                    .detail("variable", v.id.0.to_string()));
            }
        }
        if objectives.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "optimization").detail("reason", "empty_objectives"));
        }
        let seed =
            id.0 as u64 ^ ((variables.len() as u64) << 8) ^ ((feasible_set.constraints.len() as u64) << 16) ^ ((objectives.len() as u64) << 24);
        Ok(Self { id, fingerprint: fingerprint_placeholder(seed), class, variables, feasible_set, objectives, assumptions, policy, limits })
    }
}
