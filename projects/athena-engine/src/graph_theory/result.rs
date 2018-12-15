//! 图论域结果与分派。

use athena_types::Diagnostic;

use super::{
    connectivity::connected_components_l1, object::GraphNodeId, path::shortest_path_non_negative,
    property::GraphPropertyResult, request::GraphTheoryRequest,
};

/// 连通分量结果。
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectedComponentsResult {
    /// 每节点分量代表（最小 `GraphNodeId`）。
    pub labels: Vec<GraphNodeId>,
    /// 分量个数。
    pub component_count: u64,
    /// 性质合同。
    pub property: GraphPropertyResult<u64>,
}

/// 最短路结果。
#[derive(Debug, Clone, PartialEq)]
pub enum ShortestPathResult {
    /// 找到路径。
    Found {
        /// 距离。
        distance: u64,
        /// 节点序列。
        path: Vec<GraphNodeId>,
        /// 性质合同。
        property: GraphPropertyResult<u64>,
    },
    /// 不可达。
    Unreachable {
        /// 性质合同。
        property: GraphPropertyResult<()>,
    },
    /// 检测到负权环（非负域不应出现；预留）。
    NegativeCycle {
        /// 环上节点。
        cycle: Vec<GraphNodeId>,
    },
    /// 权重域不支持。
    UnsupportedWeightDomain,
    /// 资源限制。
    ResourceLimit,
    /// 部分完成。
    Incomplete,
}

/// 图论值载荷。
#[derive(Debug, Clone, PartialEq)]
pub enum GraphTheoryValue {
    /// 连通分量。
    ConnectedComponents(ConnectedComponentsResult),
    /// 最短路。
    ShortestPath(ShortestPathResult),
}

/// 图论域结果。
#[derive(Debug, Clone, PartialEq)]
pub enum GraphTheoryResult {
    /// 精确结果。
    Exact {
        /// 值。
        value: GraphTheoryValue,
    },
    /// 未求值。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

/// 执行图论请求（E0：经 [`DomainRequest::GraphTheory`] 入口）。
pub fn execute_graph_theory(request: GraphTheoryRequest) -> GraphTheoryResult {
    match request {
        GraphTheoryRequest::ConnectedComponents { graph } => {
            GraphTheoryResult::Exact { value: GraphTheoryValue::ConnectedComponents(connected_components_l1(&graph)) }
        }
        GraphTheoryRequest::ShortestPath { graph, source, target } => {
            match shortest_path_non_negative(&graph, source, target) {
                Ok(sp) => GraphTheoryResult::Exact { value: GraphTheoryValue::ShortestPath(sp) },
                Err(reason) => GraphTheoryResult::Unevaluated { reason },
            }
        }
    }
}

/// 操作名（诊断用）。
pub fn operation_name(request: &GraphTheoryRequest) -> &'static str {
    match request {
        GraphTheoryRequest::ConnectedComponents { .. } => "ConnectedComponents",
        GraphTheoryRequest::ShortestPath { .. } => "ShortestPath",
    }
}
