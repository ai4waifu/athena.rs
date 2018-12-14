//! 内存邻接图与 capability。

use crate::{
    EdgeId, GraphRevision, NodeId,
    capability::{GraphAlgorithmRequirements, GraphCapabilities},
};

/// 图方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDirection {
    /// 有向。
    Directed,
    /// 无向。
    Undirected,
}

/// 小图内存邻接表（便利实现，不是规模上限）。
#[derive(Debug, Clone)]
pub struct Graph<N, E> {
    direction: GraphDirection,
    revision: GraphRevision,
    nodes: Vec<N>,
    edges: Vec<(NodeId, NodeId, E)>,
    outgoing: Vec<Vec<EdgeId>>,
    incoming: Vec<Vec<EdgeId>>,
}

impl<N, E> Default for Graph<N, E> {
    fn default() -> Self {
        Self::new(GraphDirection::Directed)
    }
}

impl<N, E> Graph<N, E> {
    /// 创建空图。
    pub fn new(direction: GraphDirection) -> Self {
        Self {
            direction,
            revision: GraphRevision(0),
            nodes: Vec::new(),
            edges: Vec::new(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
        }
    }

    /// 方向。
    pub const fn direction(&self) -> GraphDirection {
        self.direction
    }

    /// 结构修订号。
    pub const fn revision(&self) -> GraphRevision {
        self.revision
    }

    /// 添加节点。
    pub fn add_node(&mut self, value: N) -> NodeId {
        let id = NodeId(self.nodes.len() as u64);
        self.nodes.push(value);
        self.outgoing.push(Vec::new());
        self.incoming.push(Vec::new());
        self.revision.0 += 1;
        id
    }

    /// 添加有向边；无向图同时登记反向邻接索引。
    pub fn add_edge(&mut self, source: NodeId, target: NodeId, value: E) -> Option<EdgeId> {
        let n = self.nodes.len() as u64;
        if source.0 >= n || target.0 >= n {
            return None;
        }
        let id = EdgeId(self.edges.len() as u64);
        self.edges.push((source, target, value));
        self.outgoing[source.0 as usize].push(id);
        if self.direction == GraphDirection::Directed {
            self.incoming[target.0 as usize].push(id);
        }
        else if source != target {
            self.outgoing[target.0 as usize].push(id);
        }
        self.revision.0 += 1;
        Some(id)
    }

    /// 出邻接目标（有向=出边；无向=邻接）。
    pub fn out_neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.outgoing.get(node.0 as usize).into_iter().flatten().map(move |edge| self.target_of_edge(*edge, node))
    }

    /// 入邻接列表（有向=入边源；无向=邻接）。
    pub fn in_neighbors(&self, node: NodeId) -> Vec<NodeId> {
        if self.direction == GraphDirection::Undirected {
            self.out_neighbors(node).collect()
        }
        else {
            self.incoming
                .get(node.0 as usize)
                .map(|list| list.iter().map(|edge| self.source_of_edge(*edge)).collect())
                .unwrap_or_default()
        }
    }

    /// 邻接（兼容旧 API：有向=出边；无向=无向邻接）。
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.out_neighbors(node)
    }

    /// 查找 `source → target` 的第一条边。
    pub fn find_edge(&self, source: NodeId, target: NodeId) -> Option<EdgeId> {
        self.outgoing.get(source.0 as usize).and_then(|list| {
            list.iter().copied().find(|&edge| {
                let (s, t, _) = &self.edges[edge.0 as usize];
                *s == source && *t == target
            })
        })
    }

    /// 所有边 `(source, target, edge_id)`。
    pub fn edges(&self) -> impl Iterator<Item = (NodeId, NodeId, EdgeId)> + '_ {
        self.edges.iter().enumerate().map(|(i, (s, t, _))| (*s, *t, EdgeId(i as u64)))
    }

    /// 节点数。
    pub fn node_count(&self) -> u64 {
        self.nodes.len() as u64
    }

    /// 边数。
    pub fn edge_count(&self) -> u64 {
        self.edges.len() as u64
    }

    /// 只读图视图。
    pub fn view(&self) -> GraphView<'_, N, E> {
        GraphView { graph: self }
    }

    /// 本表示的 capability 报告。
    pub fn capabilities(&self) -> GraphCapabilities {
        GraphCapabilities {
            in_memory: true,
            sorted_adjacency: false,
            reverse_adjacency: self.direction == GraphDirection::Directed,
            random_access: true,
            chunked_sequential: false,
            external_workspace: false,
        }
    }

    /// 校验是否满足算法需求。
    pub fn ensure_capabilities(&self, req: GraphAlgorithmRequirements) -> Result<(), crate::GraphError> {
        if self.capabilities().satisfies(req) {
            Ok(())
        }
        else {
            Err(crate::GraphError::CapabilityMismatch { requirement: req })
        }
    }

    fn target_of_edge(&self, edge: EdgeId, from: NodeId) -> NodeId {
        let (s, t, _) = &self.edges[edge.0 as usize];
        if self.direction == GraphDirection::Undirected && *s == from {
            *t
        }
        else if self.direction == GraphDirection::Undirected {
            *s
        }
        else {
            *t
        }
    }

    fn source_of_edge(&self, edge: EdgeId) -> NodeId {
        self.edges[edge.0 as usize].0
    }
}

/// 只读图视图（不拥有存储）。
#[derive(Debug, Clone, Copy)]
pub struct GraphView<'a, N, E> {
    graph: &'a Graph<N, E>,
}

impl<'a, N, E> GraphView<'a, N, E> {
    /// 底层图引用（不消费视图）。
    pub const fn graph_ref(&self) -> &'a Graph<N, E> {
        self.graph
    }

    /// 底层图（消费视图）。
    pub const fn graph(self) -> &'a Graph<N, E> {
        self.graph
    }

    /// 方向。
    pub const fn direction(self) -> GraphDirection {
        self.graph.direction()
    }

    /// 修订号。
    pub const fn revision(self) -> GraphRevision {
        self.graph.revision()
    }

    /// 节点数。
    pub fn node_count(self) -> u64 {
        self.graph.node_count()
    }

    /// 边数。
    pub fn edge_count(self) -> u64 {
        self.graph.edge_count()
    }

    /// 出邻接。
    pub fn out_neighbors(self, node: NodeId) -> impl Iterator<Item = NodeId> + 'a {
        self.graph.out_neighbors(node)
    }

    /// capability 报告。
    pub fn capabilities(self) -> GraphCapabilities {
        self.graph.capabilities()
    }
}
