//! 图论域结果与分派。

use athena_types::Diagnostic;

use super::{
    bipartite::bipartite_l1,
    connectivity::{connected_components_l1, strongly_connected_components_l1},
    mst::minimum_spanning_forest_l1,
    object::GraphNodeId,
    path::shortest_path_non_negative,
    property::GraphPropertyResult,
    request::GraphTheoryRequest,
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

/// 强连通分量结果。
#[derive(Debug, Clone, PartialEq)]
pub struct StronglyConnectedComponentsResult {
    /// 每节点 SCC 代表（最小 `GraphNodeId`）。
    pub labels: Vec<GraphNodeId>,
    /// SCC 个数。
    pub component_count: u64,
    /// 性质合同。
    pub property: GraphPropertyResult<u64>,
}

/// 二部性结果。
#[derive(Debug, Clone, PartialEq)]
pub enum BipartiteResult {
    /// 已证二部。
    Bipartite {
        /// 一侧。
        left: Vec<GraphNodeId>,
        /// 另一侧。
        right: Vec<GraphNodeId>,
        /// 性质合同。
        property: GraphPropertyResult<bool>,
    },
    /// 已证非二部（含奇环）。
    NotBipartite {
        /// 性质合同。
        property: GraphPropertyResult<()>,
    },
}

/// 生成树边。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanningEdge {
    /// 端点（规范化 `source ≤ target`）。
    pub source: GraphNodeId,
    /// 端点。
    pub target: GraphNodeId,
    /// 边权。
    pub weight: u64,
}

/// 最小生成森林结果。
#[derive(Debug, Clone, PartialEq)]
pub struct MinimumSpanningForestResult {
    /// 选中边。
    pub edges: Vec<SpanningEdge>,
    /// 总权。
    pub total_weight: u64,
    /// 森林中树（连通分量）个数。
    pub tree_count: u64,
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
    /// 强连通分量。
    StronglyConnectedComponents(StronglyConnectedComponentsResult),
    /// 二部性。
    Bipartite(BipartiteResult),
    /// 最小生成森林。
    MinimumSpanningForest(MinimumSpanningForestResult),
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
        GraphTheoryRequest::StronglyConnectedComponents { graph } => match strongly_connected_components_l1(&graph) {
            Ok(v) => GraphTheoryResult::Exact { value: GraphTheoryValue::StronglyConnectedComponents(v) },
            Err(reason) => GraphTheoryResult::Unevaluated { reason },
        },
        GraphTheoryRequest::Bipartite { graph } => GraphTheoryResult::Exact { value: GraphTheoryValue::Bipartite(bipartite_l1(&graph)) },
        GraphTheoryRequest::MinimumSpanningForest { graph } => match minimum_spanning_forest_l1(&graph) {
            Ok(v) => GraphTheoryResult::Exact { value: GraphTheoryValue::MinimumSpanningForest(v) },
            Err(reason) => GraphTheoryResult::Unevaluated { reason },
        },
        GraphTheoryRequest::ShortestPath { graph, source, target } => match shortest_path_non_negative(&graph, source, target) {
            Ok(sp) => GraphTheoryResult::Exact { value: GraphTheoryValue::ShortestPath(sp) },
            Err(reason) => GraphTheoryResult::Unevaluated { reason },
        },
    }
}

/// 操作名（诊断用）。
pub fn operation_name(request: &GraphTheoryRequest) -> &'static str {
    match request {
        GraphTheoryRequest::ConnectedComponents { .. } => "ConnectedComponents",
        GraphTheoryRequest::StronglyConnectedComponents { .. } => "StronglyConnectedComponents",
        GraphTheoryRequest::Bipartite { .. } => "Bipartite",
        GraphTheoryRequest::MinimumSpanningForest { .. } => "MinimumSpanningForest",
        GraphTheoryRequest::ShortestPath { .. } => "ShortestPath",
    }
}
