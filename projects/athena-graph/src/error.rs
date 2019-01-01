//! 图结构化错误。

use athena_ndarray::ArrayError;

use crate::capability::GraphAlgorithmRequirements;

/// 图错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// 节点计数溢出。
    NodeOverflow,
    /// offsets 长度不匹配。
    OffsetLength,
    /// CSR/CSC 边界非法。
    Boundary,
    /// offsets 非单调或不在 `[0, edges]`。
    OffsetNonMonotonic {
        /// 出错下标。
        index: u64,
        /// 前一 offset。
        prev: u64,
        /// 当前 offset。
        cur: u64,
    },
    /// 声称 sorted 但邻接无序。
    AdjacencyUnsorted {
        /// 节点。
        node: u64,
        /// indices 偏移。
        offset: u64,
    },
    /// 非法节点。
    InvalidNode,
    /// 非法邻接目标。
    InvalidTarget,
    /// 边列表与节点数不一致。
    InvalidEdgeList,
    /// 无向图不能导出 CSR。
    UndirectedCsr,
    /// 算法 capability 不满足。
    CapabilityMismatch {
        /// 未满足的需求。
        requirement: GraphAlgorithmRequirements,
    },
    /// 下层 ndarray 错误。
    Array(ArrayError),
    /// 图含环，无法拓扑排序。
    CycleDetected,
    /// 无向图不支持拓扑排序。
    UndirectedTopo,
}

impl From<ArrayError> for GraphError {
    fn from(value: ArrayError) -> Self {
        Self::Array(value)
    }
}
