//! Operational 状态：frontier · 非语义缓存 · 暂存超边 / OuterCandidate 池 / determinacy。

use crate::reasoning::mgraph::{
    admission::OuterCandidate,
    cache::result::ResultCache,
    core::types::{DeterminacyState, HyperEdge, SolverFrontier},
    obligation::{ObligationIndex, ProofObligation, QueuedPlan},
};

/// 操作层状态（非单调；可驱逐、可重建）。
///
/// **不**实现 [`Clone`]（`result_cache` 含多项式 owning 载荷）。
#[derive(Debug, Default, PartialEq)]
pub struct OperationalState {
    /// Solver 调度前沿。
    pub frontier: SolverFrontier,
    /// 非语义结果缓存。
    pub result_cache: ResultCache,
    /// 暂存超边（未经 verifier 的候选；可 drain 进 [`Self::outer_candidates`]）。
    pub hyper_edges: Vec<HyperEdge>,
    /// 已从 hyper-edge 映射、仍未经验证的外候选（**不得**直接进入 ExactUF）。
    pub outer_candidates: Vec<OuterCandidate>,
    /// 等待接纳时 Reflector 唤醒的挂起 `ProofObligation`。
    pub obligation_index: ObligationIndex,
    /// 由 Reflector `NeedComputation` 入队的 `DomainPlan`（尚未执行）。
    pub pending_plans: Vec<QueuedPlan>,
    /// 为 frontier 恢复而保留的未决义务。
    pub resume_queue: Vec<ProofObligation>,
    /// 全局 determinacy 占位（后续改为 per-claim）。
    pub determinacy: DeterminacyState,
}

impl OperationalState {
    /// 空 operational 状态。
    pub fn new() -> Self {
        Self::default()
    }
}
