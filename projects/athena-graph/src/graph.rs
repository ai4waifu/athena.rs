//! 内存邻接图与 capability。

use crate::{EdgeId, NodeId};

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
    nodes: Vec<N>,
    edges: Vec<(NodeId, NodeId, E)>,
    outgoing: Vec<Vec<EdgeId>>,
}

impl<N, E> Default for Graph<N, E> {
    fn default() -> Self {
        Self::new(GraphDirection::Directed)
    }
}

impl<N, E> Graph<N, E> {
    /// 创建空图。
    pub const fn new(direction: GraphDirection) -> Self {
        Self {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
            outgoing: Vec::new(),
        }
    }

    /// 方向。
    pub const fn direction(&self) -> GraphDirection {
        self.direction
    }

    /// 添加节点。
    pub fn add_node(&mut self, value: N) -> NodeId {
        let id = NodeId(self.nodes.len() as u64);
        self.nodes.push(value);
        self.outgoing.push(Vec::new());
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
        if self.direction == GraphDirection::Undirected && source != target {
            self.outgoing[target.0 as usize].push(id);
        }
        Some(id)
    }

    /// 出邻接目标。
    pub fn neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.outgoing
            .get(node.0 as usize)
            .into_iter()
            .flatten()
            .map(move |edge| {
                let (s, t, _) = &self.edges[edge.0 as usize];
                if self.direction == GraphDirection::Undirected && *s == node {
                    *t
                } else if self.direction == GraphDirection::Undirected {
                    *s
                } else {
                    *t
                }
            })
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
}

/// 只读图视图（不拥有存储）。
#[derive(Debug, Clone, Copy)]
pub struct GraphView<'a, N, E> {
    graph: &'a Graph<N, E>,
}

impl<'a, N, E> GraphView<'a, N, E> {
    /// 底层图。
    pub const fn graph(self) -> &'a Graph<N, E> {
        self.graph
    }

    /// 方向。
    pub const fn direction(self) -> GraphDirection {
        self.graph.direction()
    }

    /// 节点数。
    pub fn node_count(self) -> u64 {
        self.graph.node_count()
    }

    /// 边数。
    pub fn edge_count(self) -> u64 {
        self.graph.edge_count()
    }
}

/// 图算法对工作集与 storage 的要求（首轮 capability 合同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphAlgorithmRequirements {
    /// 必须完整图驻留内存。
    InMemoryOnly,
    /// 支持顺序扫描边/邻接。
    ChunkedSequential,
    /// frontier / visited 可落外存。
    ExternalWorkspace,
}
