//! M-Graph 核心类型（骨架）。

use athena_types::{AssumptionSetId, TermId};

use crate::reasoning::mgraph::core::refs::PredicateId;

/// 能力 provider 身份（M-Graph / capability registry 共享）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityProviderId(pub u32);

/// 等价类划分（`TermId` → 代表元）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EquivalenceClasses {
    /// 父子指针（骨架：空表）。
    pub parent: Vec<(TermId, TermId)>,
}

impl EquivalenceClasses {
    /// Owning 复制（Living `31`：仅 `TermId` 句柄对）。
    pub fn owning_copy(&self) -> Self {
        Self { parent: self.parent.clone() }
    }
}

/// 精确性层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExactnessLevel {
    /// 精确。
    #[default]
    Exact,
    /// 条件精确。
    Conditional,
    /// 数值。
    Numeric,
    /// 启发式。
    Heuristic,
    /// 未知。
    Unknown,
}

/// 确定性状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeterminacyState {
    /// 精确性。
    pub exactness: ExactnessLevel,
    /// 附着假设。
    pub assumptions: Option<AssumptionSetId>,
}

/// 确定性保证条目。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeterminacyGuarantee {
    /// 作用的项。
    pub term: TermId,
    /// 精确性。
    pub exactness: ExactnessLevel,
}

/// 超边（多参数联合约束）。
///
/// Living `26`：用 [`PredicateId`] 标识关系，禁止任意 `String` 标签。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct HyperEdge {
    /// 参与项。
    pub nodes: Vec<TermId>,
    /// 语义谓词。
    pub predicate: PredicateId,
}

impl HyperEdge {
    /// Owning 复制（Living `31`：仅 `TermId` / `PredicateId`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            predicate: self.predicate,
        }
    }
}

/// 重写 / 求解 witness。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct RewriteWitness {
    /// 能力 provider。
    pub provider: CapabilityProviderId,
    /// 输入。
    pub inputs: Vec<TermId>,
    /// 输出。
    pub outputs: Vec<TermId>,
}

impl RewriteWitness {
    /// Owning 复制（Living `31`：仅句柄向量）。
    pub fn owning_copy(&self) -> Self {
        Self {
            provider: self.provider,
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
        }
    }
}

/// 等式 witness。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct EqualityWitness {
    /// 左。
    pub left: TermId,
    /// 右。
    pub right: TermId,
    /// 支撑 witness。
    pub witness: RewriteWitness,
}

impl EqualityWitness {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            left: self.left,
            right: self.right,
            witness: self.witness.owning_copy(),
        }
    }
}

/// 求解候选。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct SolverCandidate {
    /// 能力 provider。
    pub provider: CapabilityProviderId,
    /// 根项。
    pub roots: Vec<TermId>,
}

impl SolverCandidate {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            provider: self.provider,
            roots: self.roots.clone(),
        }
    }
}

/// 调度评分（量化整数 + 稳定 tie-breaker；占位策略仍可用浮点估计推导）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SolverScore {
    /// 量化总分（越大越优先）。
    pub total: i64,
    /// 稳定 tie-breaker（solver / roots 指纹；与平台浮点顺序无关）。
    pub tie_breaker: u64,
}

impl SolverScore {
    /// 全序比较键。
    pub fn ordering_key(self) -> (i64, u64) {
        (self.total, self.tie_breaker)
    }
}

/// 求解前沿。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default, PartialEq)]
pub struct SolverFrontier {
    /// 候选。
    pub candidates: Vec<SolverCandidate>,
    /// 评分（与 candidates 对齐）。
    pub scores: Vec<SolverScore>,
}

impl SolverFrontier {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            candidates: self.candidates.iter().map(SolverCandidate::owning_copy).collect(),
            scores: self.scores.clone(),
        }
    }
}
