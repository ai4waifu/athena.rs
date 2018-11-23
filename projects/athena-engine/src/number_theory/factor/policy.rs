//! 分解策略与执行预算。

use crate::number_theory::value::FactorComponent;

/// 素性 / 证明要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProofRequirement {
    /// 仅接受确定素因子。
    Proven,
    /// 允许概率素因子。
    #[default]
    Probable,
}

/// 允许的分解算法阶段（bootstrap 仅 trial）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactorAlgorithms {
    /// 试除。
    pub trial: bool,
    /// Pollard rho（未实现）。
    pub pollard_rho: bool,
    /// Pollard p-1（未实现）。
    pub pollard_p1: bool,
    /// ECM（未实现）。
    pub ecm: bool,
}

impl FactorAlgorithms {
    /// Gate 1 默认：仅试除。
    pub fn bootstrap() -> Self {
        Self {
            trial: true,
            pollard_rho: false,
            pollard_p1: false,
            ecm: false,
        }
    }
}

impl Default for FactorAlgorithms {
    fn default() -> Self {
        Self::bootstrap()
    }
}

/// 分解策略（证明要求、算法、可复现性）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactorPolicy {
    /// 因子素性证明要求。
    pub proof_requirement: ProofRequirement,
    /// 允许的阶段。
    pub algorithms: FactorAlgorithms,
    /// 确定性随机种子（后续 rho/ECM）。
    pub deterministic_seed: Option<u64>,
    /// 并行度占位（0 = 默认）。
    pub parallelism: u32,
}

impl Default for FactorPolicy {
    fn default() -> Self {
        Self {
            proof_requirement: ProofRequirement::Probable,
            algorithms: FactorAlgorithms::bootstrap(),
            deterministic_seed: None,
            parallelism: 0,
        }
    }
}

/// 分解执行预算。`max_input_bits` 为输入拒绝阈值，**不是**已消耗 wall/CPU。
///
/// 命名避开 `athena_numeric::ExecutionBudget`（数值 limb 预算）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactorExecutionBudget {
    /// 试除上界（含）。
    pub max_trial: u64,
    /// 输入绝对值比特上限；超出 → 拒绝执行（`input_rejected`）。
    pub max_input_bits: u32,
    /// 算法步数上限（未实现续算时可为 `None`）。
    pub max_steps: Option<u64>,
    /// wall 时间上限毫秒（未接线）。
    pub max_time_ms: Option<u64>,
}

impl Default for FactorExecutionBudget {
    fn default() -> Self {
        Self {
            max_trial: 1_000_000,
            max_input_bits: 256,
            max_steps: None,
            max_time_ms: None,
        }
    }
}

/// 分解资源合同（策略 + 预算）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactorLimits {
    /// 策略。
    pub policy: FactorPolicy,
    /// 预算。
    pub budget: FactorExecutionBudget,
}

impl FactorLimits {
    /// 试除上界（含）。
    pub fn max_trial(&self) -> u64 {
        self.budget.max_trial
    }

    /// 输入比特上限。
    pub fn max_bits(&self) -> u32 {
        self.budget.max_input_bits
    }
}

impl Default for FactorLimits {
    fn default() -> Self {
        Self {
            policy: FactorPolicy::default(),
            budget: FactorExecutionBudget::default(),
        }
    }
}

/// 可续算分解前沿（后续 ECM/QS 接入）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FactorFrontier {
    /// 已找到的因子。
    pub factors_found: Vec<FactorComponent>,
    /// 尚未分解的余因子。
    pub unresolved_cofactors: Vec<athena_numeric::Integer>,
}
