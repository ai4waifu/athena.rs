//! 零拷贝逻辑图视图：反向 · 诱导子图 · 边过滤。

use std::collections::HashSet;

use crate::{EdgeId, Graph, GraphDirection, GraphView, NodeId};

/// 有向图反向视图（沿原图入边前进）。
#[derive(Debug, Clone, Copy)]
pub struct ReversedGraphView<'a, N, E> {
    graph: &'a Graph<N, E>,
}

impl<'a, N, E> ReversedGraphView<'a, N, E> {
    /// 包装有向图。
    pub fn new(graph: &'a Graph<N, E>) -> Option<Self> {
        if graph.direction() == GraphDirection::Directed { Some(Self { graph }) } else { None }
    }

    /// 方向（恒为有向）。
    pub const fn direction(self) -> GraphDirection {
        GraphDirection::Directed
    }

    /// 节点数。
    pub fn node_count(self) -> u64 {
        self.graph.node_count()
    }

    /// 反向后的出邻接 = 原图入邻接。
    pub fn neighbors(self, node: NodeId) -> impl Iterator<Item = NodeId> + 'a {
        self.graph.in_neighbors(node).into_iter()
    }
}

/// 诱导子图视图（仅保留给定节点及其内部边）。
#[derive(Debug, Clone)]
pub struct InducedSubgraphView<'a, N, E> {
    graph: &'a Graph<N, E>,
    nodes: HashSet<u64>,
}

impl<'a, N, E> InducedSubgraphView<'a, N, E> {
    /// 构造诱导子图；空集表示空图投影。
    pub fn new(graph: &'a Graph<N, E>, keep: impl IntoIterator<Item = NodeId>) -> Self {
        Self { graph, nodes: keep.into_iter().map(|n| n.0).collect() }
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
#[derive(Debug, Clone, Copy)]
pub struct EdgeFilteredView<'a, N, E, F> {
    graph: &'a Graph<N, E>,
    predicate: F,
}

impl<'a, N, E, F> EdgeFilteredView<'a, N, E, F>
where
    F: Copy + Fn(NodeId, NodeId, EdgeId) -> bool,
{
    /// 包装图引用与边谓词。
    pub fn new(graph: &'a Graph<N, E>, predicate: F) -> Self {
        Self { graph, predicate }
    }

    /// 从 [`GraphView`] 构造。
    pub fn from_view(view: GraphView<'a, N, E>, predicate: F) -> Self {
        Self::new(view.graph_ref(), predicate)
    }

    /// 过滤后的邻接（沿保留边前进）。
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.graph
            .out_neighbors(node)
            .filter(move |&target| self.graph.find_edge(node, target).is_some_and(|edge| (self.predicate)(node, target, edge)))
    }

    /// 节点数（与底图相同）。
    pub fn node_count(self) -> u64 {
        self.graph.node_count()
    }
}
