//! Operational 状态：frontier · 非语义缓存 · 暂存超边 / determinacy。

use crate::reasoning::mgraph::{
    cache::result::ResultCache,
    core::types::{DeterminacyState, HyperEdge, SolverCandidate, SolverFrontier},
};

/// 操作层状态（非单调；可驱逐、可重建）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OperationalState {
    /// Solver 调度前沿。
    pub frontier: SolverFrontier,
    /// 非语义结果缓存。
    pub result_cache: ResultCache,
    /// 暂存超边（未经 verifier 的候选）。
    pub hyper_edges: Vec<HyperEdge>,
    /// 全局 determinacy 占位（后续改为 per-claim）。
    pub determinacy: DeterminacyState,
}

impl OperationalState {
    /// 空 operational 状态。
    pub fn new() -> Self {
        Self::default()
    }
}
