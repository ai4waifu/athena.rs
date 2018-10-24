//! M-Graph 状态容器。

use super::{
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
    /// witnesses。
    pub witnesses: Vec<RewriteWitness>,
    /// solver 前沿。
    pub frontier: SolverFrontier,
    /// 多项式子图（缓存 + witness）。
    pub polynomial: PolynomialMGraphStore,
}

impl MGraphState {
    /// 空状态。
    pub fn new() -> Self {
        Self::default()
    }
}
