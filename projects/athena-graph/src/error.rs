//! 图结构化错误。

use athena_ndarray::ArrayError;

/// 图错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// 节点计数溢出。
    NodeOverflow,
    /// offsets 长度不匹配。
    OffsetLength,
    /// CSR 边界非法。
    Boundary,
    /// 非法节点。
    InvalidNode,
    /// 非法邻接目标。
    InvalidTarget,
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
