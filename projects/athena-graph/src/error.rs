//! 图结构化错误。

use athena_ndarray::ArrayError;

use crate::storage::GraphAlgorithmRequirements;

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
    /// 派生 CSC 相对源 CSR 已过期（revision / graph_id 不匹配）。
    StaleCsc {
        /// 派生时记录的源修订。
        derived_from: crate::GraphRevision,
        /// 当前 CSR 修订（若可知）。
        current: Option<crate::GraphRevision>,
    },
    /// 属性列长度与 nodes/edges 不匹配。
    PropertyLengthMismatch {
        /// 期望长度。
        expected: u64,
        /// 实际长度。
        actual: u64,
    },
    /// 下层 ndarray 错误。
    Array(ArrayError),
    /// 图含环，无法拓扑排序。
    CycleDetected,
    /// 无向图不支持拓扑排序。
    UndirectedTopo,
    /// 引用所属图与当前图不一致。
    WrongGraph {
        /// 引用上的图身份。
        expected: crate::GraphId,
        /// 当前图身份。
        actual: crate::GraphId,
    },
    /// 引用绑定的 revision 已过期。
    StaleRef {
        /// 引用上的修订。
        expected: crate::GraphRevision,
        /// 当前修订。
        actual: crate::GraphRevision,
    },
    /// 视图绑定的底图 revision 已过期（或底图身份不一致时先报 [`Self::WrongGraph`]）。
    StaleView {
        /// 视图创建时的底图修订。
        expected: crate::GraphRevision,
        /// 当前底图修订。
        actual: crate::GraphRevision,
    },
    /// [`crate::ViewNodeRef`] / [`crate::ViewEdgeRef`] 与当前视图指纹不一致。
    WrongView {
        /// 引用上的视图指纹。
        expected: crate::ViewFingerprint,
        /// 当前视图指纹。
        actual: crate::ViewFingerprint,
    },
}

impl From<ArrayError> for GraphError {
    fn from(value: ArrayError) -> Self {
        Self::Array(value)
    }
}
