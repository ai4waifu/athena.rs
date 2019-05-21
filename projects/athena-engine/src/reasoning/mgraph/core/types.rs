//! M-Graph 核心类型（骨架）。

use athena_types::{AssumptionSetId, ExprId};

/// 求解器 id（M-Graph / solver 共享）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SolverId(pub u32);

/// 等价类划分（`ExprId` → 代表元）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquivalenceClasses {
    /// 父子指针（骨架：空表）。
    pub parent: Vec<(ExprId, ExprId)>,
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeterminacyState {
    /// 精确性。
    pub exactness: ExactnessLevel,
    /// 附着假设。
    pub assumptions: Option<AssumptionSetId>,
}

/// 确定性保证条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterminacyGuarantee {
    /// 作用的项。
    pub term: ExprId,
    /// 精确性。
    pub exactness: ExactnessLevel,
}

/// 超边（多参数联合约束）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperEdge {
    /// 参与项。
    pub nodes: Vec<ExprId>,
    /// 标签。
    pub label: String,
}

/// 重写 / 求解 witness。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteWitness {
    /// 求解器。
    pub solver: SolverId,
    /// 输入。
    pub inputs: Vec<ExprId>,
    /// 输出。
    pub outputs: Vec<ExprId>,
}

/// 等式 witness。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EqualityWitness {
    /// 左。
    pub left: ExprId,
    /// 右。
    pub right: ExprId,
    /// 支撑 witness。
    pub witness: RewriteWitness,
}

/// 求解候选。
#[derive(Debug, Clone, PartialEq)]
pub struct SolverCandidate {
    /// 求解器。
    pub solver: SolverId,
    /// 根项。
    pub roots: Vec<ExprId>,
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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SolverFrontier {
    /// 候选。
    pub candidates: Vec<SolverCandidate>,
    /// 评分（与 candidates 对齐）。
    pub scores: Vec<SolverScore>,
}
