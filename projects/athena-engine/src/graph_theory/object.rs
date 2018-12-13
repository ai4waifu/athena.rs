//! 图论对象模型（Living `12` 骨架）。

use athena_graph::GraphDirection;

/// 图节点身份（`athena-graph` wire 类型）。
pub type GraphNodeId = athena_graph::NodeId;

/// 图句柄（bootstrap：内存图 id + 节点数；后续可接外存 view）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphHandle {
    /// 稳定句柄 id（Session 内由调用方分配）。
    pub id: u64,
    /// 逻辑节点数。
    pub node_count: u64,
}

/// 权重语义域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeightDomain {
    /// 无权图：最短路按边数计（边权视为 1）。
    Unweighted,
    /// 非负整数权。
    NonNegativeInteger,
}

/// 图语义（方向 · 自环 · 权重域）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphSemantics {
    /// 有向 / 无向。
    pub direction: GraphDirection,
    /// 是否允许自环。
    pub allows_self_loops: bool,
    /// 边权域。
    pub weight_domain: WeightDomain,
}

impl Default for GraphSemantics {
    fn default() -> Self {
        Self {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        }
    }
}

/// 数学表示标签（非证明）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphPresentation {
    /// 内存邻接表。
    AdjacencyList,
    /// CSR 视图（经 conversion 挂载，后续扩展）。
    CsrView,
}

/// 假设上下文占位（完整 assumption solver 外置）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphAssumptions {
    /// 附着假设集 id（0 = 无条件）。
    pub assumption_set_id: u64,
}

/// 构造来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphProvenance {
    /// 调用方直接构造。
    Explicit,
    /// 自邻接表导入。
    FromEdgeList,
}

/// 内存图载体（L1 bootstrap）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGraph {
    /// 语义。
    pub semantics: GraphSemantics,
    /// 边 `(source, target, weight)`；无权时 weight 忽略（算法按 1 计）。
    pub edges: Vec<(GraphNodeId, GraphNodeId, u64)>,
}

impl MemoryGraph {
    /// 节点数（由边推断的最小上界 + 1，或显式给定）。
    pub fn node_count(&self) -> u64 {
        self.edges
            .iter()
            .flat_map(|(s, t, _)| [s.0, t.0])
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }

    /// 物化为 [`athena_graph::Graph`]（边权存于节点 payload 占位，遍历用 semantics）。
    pub fn to_athena_graph(&self) -> athena_graph::Graph<(), u64> {
        let n = self.node_count();
        let mut g = athena_graph::Graph::new(self.semantics.direction);
        for _ in 0..n {
            g.add_node(());
        }
        for (s, t, w) in &self.edges {
            if self.semantics.allows_self_loops || s != t {
                g.add_edge(*s, *t, *w);
            }
        }
        g
    }
}

/// 图论领域对象。
#[derive(Debug, Clone, PartialEq)]
pub struct GraphObject {
    /// 句柄。
    pub handle: GraphHandle,
    /// 语义。
    pub semantics: GraphSemantics,
    /// 表示。
    pub presentation: GraphPresentation,
    /// 假设。
    pub assumptions: GraphAssumptions,
    /// 来源。
    pub provenance: GraphProvenance,
    /// 内存载体。
    pub memory: MemoryGraph,
}

impl GraphObject {
    /// 从边列表构造（节点 id 须从 0 连续或稀疏，由 `node_count` 指定上界）。
    pub fn from_edges(
        handle: GraphHandle,
        semantics: GraphSemantics,
        edges: Vec<(GraphNodeId, GraphNodeId, u64)>,
    ) -> Self {
        Self {
            handle,
            semantics,
            presentation: GraphPresentation::AdjacencyList,
            assumptions: GraphAssumptions::default(),
            provenance: GraphProvenance::FromEdgeList,
            memory: MemoryGraph { semantics, edges },
        }
    }
}
