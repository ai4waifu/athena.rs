//! 零拷贝逻辑图视图：反向 · 诱导子图 · 边过滤（含 base mapping）。

use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use crate::{
    direction::GraphDirection,
    graph::{Graph, GraphView},
    id::{EdgeId, GraphId, GraphRevision, NodeId},
    semantics::{ViewMapping, ViewTransform},
};

fn hash_nodes(nodes: &HashSet<u64>) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut v: Vec<u64> = nodes.iter().copied().collect();
    v.sort_unstable();
    v.hash(&mut hasher);
    hasher.finish()
}

/// 有向图反向视图（沿原图入边前进）。
#[derive(Debug, Clone)]
pub struct ReversedGraphView<'a, N, E> {
    graph: &'a Graph<N, E>,
    mapping: ViewMapping,
}

impl<'a, N, E> ReversedGraphView<'a, N, E> {
    /// 包装有向图。
    pub fn new(graph: &'a Graph<N, E>) -> Option<Self> {
        if graph.direction() != GraphDirection::Directed {
            return None;
        }
        let mapping = ViewMapping::new(graph.id(), graph.revision(), ViewTransform::Reversed, 0);
        Some(Self { graph, mapping })
    }

    /// 底图映射合同。
    pub fn mapping(&self) -> &ViewMapping {
        &self.mapping
    }

    /// view 节点 → base 节点（恒等）。
    pub fn map_node_to_base(&self, node: NodeId) -> Option<NodeId> {
        (node.0 < self.graph.node_count()).then_some(node)
    }

    /// view 边 → base 边（恒等；定向在呈现层翻转）。
    pub fn map_edge_to_base(&self, edge: EdgeId) -> Option<EdgeId> {
        (edge.0 < self.graph.edge_count()).then_some(edge)
    }

    /// 底图 id。
    pub const fn base_graph_id(&self) -> GraphId {
        self.mapping.base_graph_id
    }

    /// 底图 revision。
    pub const fn base_revision(&self) -> GraphRevision {
        self.mapping.base_revision
    }

    /// 方向（恒为有向）。
    pub const fn direction(&self) -> GraphDirection {
        GraphDirection::Directed
    }

    /// 节点数。
    pub fn node_count(&self) -> u64 {
        self.graph.node_count()
    }

    /// 反向后的出邻接 = 原图入邻接。
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.graph.in_neighbors(node).into_iter()
    }
}

/// 诱导子图视图（仅保留给定节点及其内部边）。
#[derive(Debug, Clone)]
pub struct InducedSubgraphView<'a, N, E> {
    graph: &'a Graph<N, E>,
    nodes: HashSet<u64>,
    mapping: ViewMapping,
}

impl<'a, N, E> InducedSubgraphView<'a, N, E> {
    /// 构造诱导子图；空集表示空图投影。节点 id 仍为 base local id。
    pub fn new(graph: &'a Graph<N, E>, keep: impl IntoIterator<Item = NodeId>) -> Self {
        let nodes: HashSet<u64> = keep.into_iter().map(|n| n.0).collect();
        let transform_hash = hash_nodes(&nodes);
        let mapping = ViewMapping::new(graph.id(), graph.revision(), ViewTransform::Induced, transform_hash);
        Self { graph, nodes, mapping }
    }

    /// 底图映射合同。
    pub fn mapping(&self) -> &ViewMapping {
        &self.mapping
    }

    /// view 节点 → base（恒等过滤）。
    pub fn map_node_to_base(&self, node: NodeId) -> Option<NodeId> {
        self.contains(node).then_some(node)
    }

    /// view 边 → base：两端均在保留集内时返回同一 `EdgeId`。
    pub fn map_edge_to_base(&self, edge: EdgeId) -> Option<EdgeId> {
        let (s, t) = self.graph.edge_endpoints(edge)?;
        (self.contains(s) && self.contains(t)).then_some(edge)
    }

    /// 节点是否保留。
    pub fn contains(&self, node: NodeId) -> bool {
        self.nodes.contains(&node.0)
    }

    /// 保留节点数。
    pub fn node_count(&self) -> u64 {
        self.nodes.len() as u64
    }

    /// 方向与底图一致。
    pub const fn direction(&self) -> GraphDirection {
        self.graph.direction()
    }

    /// 诱导子图上的邻接（两端均在保留集内）。
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let keep = &self.nodes;
        self.graph.out_neighbors(node).filter(move |target| keep.contains(&node.0) && keep.contains(&target.0))
    }
}

/// 边过滤视图（保留满足谓词的边）。
#[derive(Debug, Clone)]
pub struct EdgeFilteredView<'a, N, E, F> {
    graph: &'a Graph<N, E>,
    predicate: F,
    mapping: ViewMapping,
}

impl<'a, N, E, F> EdgeFilteredView<'a, N, E, F>
where
    F: Copy + Fn(NodeId, NodeId, EdgeId) -> bool,
{
    /// 包装图引用与边谓词。
    pub fn new(graph: &'a Graph<N, E>, predicate: F) -> Self {
        let mapping = ViewMapping::new(graph.id(), graph.revision(), ViewTransform::EdgeFiltered, 0);
        Self { graph, predicate, mapping }
    }

    /// 从 [`GraphView`] 构造。
    pub fn from_view(view: GraphView<'a, N, E>, predicate: F) -> Self {
        Self::new(view.graph_ref(), predicate)
    }

    /// 底图映射合同。
    pub fn mapping(&self) -> &ViewMapping {
        &self.mapping
    }

    /// view 节点 → base（恒等）。
    pub fn map_node_to_base(&self, node: NodeId) -> Option<NodeId> {
        (node.0 < self.graph.node_count()).then_some(node)
    }

    /// 仅当谓词接受该边时映回 base。
    pub fn map_edge_to_base(&self, edge: EdgeId) -> Option<EdgeId> {
        let (s, t) = self.graph.edge_endpoints(edge)?;
        (self.predicate)(s, t, edge).then_some(edge)
    }

    /// 过滤后的邻接（沿保留边前进）。
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.graph
            .out_neighbors(node)
            .filter(move |&target| self.graph.find_edge(node, target).is_some_and(|edge| (self.predicate)(node, target, edge)))
    }

    /// 节点数（与底图相同）。
    pub fn node_count(&self) -> u64 {
        self.graph.node_count()
    }
}
