//! 分解策略与执行预算。

use crate::domains::number_theory::value::FactorComponent;
use athena_numeric::Integer;

/// 素性 / 证明要求。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ProofRequirement {
    /// 仅接受确定素因子。
    Proven,
    /// 允许概率素因子。
    #[default]
    Probable,
}

/// 允许的分解算法阶段。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FactorAlgorithms {
    /// 试除。
    pub trial: bool,
    /// Pollard ρ。
    pub pollard_rho: bool,
    /// Pollard p−1。
    pub pollard_p1: bool,
    /// ECM（Montgomery 第 1 阶段）。
    pub ecm: bool,
    /// QS 引导实现（Fermat 近距分解）。
    pub quadratic_sieve: bool,
}

impl FactorAlgorithms {
    /// 引导默认：仅试除。
    pub fn bootstrap() -> Self {
        Self { trial: true, pollard_rho: false, pollard_p1: false, ecm: false, quadratic_sieve: false }
    }

    /// 试除 → rho → p−1 → ECM → QS。
    pub fn with_pipeline() -> Self {
        Self { trial: true, pollard_rho: true, pollard_p1: true, ecm: true, quadratic_sieve: true }
    }
}

impl Default for FactorAlgorithms {
    fn default() -> Self {
        Self::bootstrap()
    }
}

/// 分解策略（证明要求、算法、可复现性）。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FactorPolicy {
    /// 因子素性证明要求。
    pub proof_requirement: ProofRequirement,
    /// 允许的阶段。
    pub algorithms: FactorAlgorithms,
    /// 确定性随机种子（rho / p−1 / ECM）。
    pub deterministic_seed: Option<u64>,
    /// 并行度占位（0 = 默认）。
    pub parallelism: u32,
    /// Pollard p−1 / ECM stage-1 的 `B1` 上界。
    pub stage1_b1: u32,
    /// ECM 尝试曲线数。
    pub ecm_curves: u32,
}

impl Default for FactorPolicy {
    fn default() -> Self {
        Self {
            proof_requirement: ProofRequirement::Probable,
            algorithms: FactorAlgorithms::bootstrap(),
            deterministic_seed: None,
            parallelism: 0,
            stage1_b1: 200,
            ecm_curves: 8,
        }
    }
}

/// 分解执行预算。`max_input_bits` 为输入拒绝阈值，**不是**已消耗 wall/CPU。
///
/// 命名避开 `athena_numeric::ExecutionBudget`（数值 limb 预算）。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FactorExecutionBudget {
    /// 试除上界（含）。
    pub max_trial: u64,
    /// 输入绝对值比特上限；超出 → 拒绝执行（`input_rejected`）。
    pub max_input_bits: u32,
    /// 算法步数上限（触及后标记 `resource_exhausted`）。
    pub max_steps: Option<u64>,
    /// wall 时间上限毫秒（未接线）。
    pub max_time_ms: Option<u64>,
}

impl Default for FactorExecutionBudget {
    fn default() -> Self {
        Self { max_trial: 1_000_000, max_input_bits: 256, max_steps: None, max_time_ms: None }
    }
}

/// 分解资源合同（策略 + 预算）。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
        Self { policy: FactorPolicy::default(), budget: FactorExecutionBudget::default() }
    }
}

/// 可续算分解前沿。
#[derive(Debug, PartialEq, Eq)]
pub struct FactorFrontier {
    /// 单位（`±1`）。
    pub unit: Integer,
    /// 已找到的因子。
    pub factors_found: Vec<FactorComponent>,
    /// 尚未分解的余因子。
    pub unresolved_cofactors: Vec<Integer>,
    /// 已消耗算法步数。
    pub steps_used: u64,
    /// 是否因预算耗尽而暂停。
    pub resource_exhausted: bool,
}

impl Default for FactorFrontier {
    fn default() -> Self {
        Self { unit: Integer::one(), factors_found: Vec::new(), unresolved_cofactors: Vec::new(), steps_used: 0, resource_exhausted: false }
    }
}

impl FactorFrontier {
    /// 是否仍有未解决余因子。
    pub fn is_pending(&self) -> bool {
        self.unresolved_cofactors.iter().any(|c| *c > Integer::one())
    }
}
