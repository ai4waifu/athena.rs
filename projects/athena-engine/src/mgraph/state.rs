//! M-Graph 状态容器。

use super::{
    claim::VerifiedClaim,
    polynomial::PolynomialMGraphStore,
    types::{DeterminacyState, EquivalenceClasses, HyperEdge, RewriteWitness, SolverFrontier},
};

/// M-Graph 状态。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MGraphState {
    /// 等价划分。
    pub classes: EquivalenceClasses,
    /// 确定性。
    pub determinacy: DeterminacyState,
    /// 超边。
    pub hyper_edges: Vec<HyperEdge>,
    /// witnesses（仅 admission 通过的 rewrite 边）。
    pub witnesses: Vec<RewriteWitness>,
    /// 已验证事实日志（append-only semantic core 骨架）。
    pub verified_claims: Vec<VerifiedClaim>,
    /// solver 前沿。
    pub frontier: SolverFrontier,
    /// 多项式子图（结果缓存；witness 仅对已接纳项）。
    pub polynomial: PolynomialMGraphStore,
}

impl MGraphState {
    /// 空状态。
    pub fn new() -> Self {
        Self::default()
    }
}
