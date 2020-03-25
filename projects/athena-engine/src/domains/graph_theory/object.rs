//! 图论对象模型。

use athena_graph::{GraphBuilder, GraphDirection, GraphId, GraphRevision, GraphSnapshot, MultiplicityPolicy, RepresentationId, SelfLoopDegree};

/// 图节点身份（`athena-graph` wire 类型）。
pub type GraphNodeId = athena_graph::NodeId;

/// 图句柄（Session 内稳定 id + 节点数）。
///
/// `id` 与 [`GraphSnapshot::graph_id`] 对齐（`GraphId::from_raw(id)`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphHandle {
    /// 稳定句柄 id（Session 内由调用方分配；等于逻辑 [`GraphId`]）。
    pub id: u64,
    /// 逻辑节点数。
    pub node_count: u64,
}

impl GraphHandle {
    /// 对应逻辑图身份。
    pub const fn graph_id(self) -> GraphId {
        GraphId::from_raw(self.id)
    }
}

/// 权重语义域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeightDomain {
    /// 无权图：最短路按边数计（边权视为 1）。
    Unweighted,
    /// 非负整数权。
    NonNegativeInteger,
}

/// 图论领域语义（结构合同 + 权重域）。
///
/// 结构部分经 [`Self::to_structural`] 投影为 `athena_graph::GraphSemantics`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphDomainSemantics {
    /// 有向 / 无向。
    pub direction: GraphDirection,
    /// 是否允许自环。
    pub allows_self_loops: bool,
    /// 是否允许平行边。
    pub allows_parallel_edges: bool,
    /// 边权域。
    pub weight_domain: WeightDomain,
}

impl Default for GraphDomainSemantics {
    fn default() -> Self {
        Self {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            allows_parallel_edges: false,
            weight_domain: WeightDomain::Unweighted,
        }
    }
}

impl GraphDomainSemantics {
    /// 便捷构造（简单图默认）。
    pub const fn new(direction: GraphDirection, weight_domain: WeightDomain) -> Self {
        Self { direction, allows_self_loops: false, allows_parallel_edges: false, weight_domain }
    }

    /// 投影为 `athena-graph` 结构语义。
    pub const fn to_structural(self) -> athena_graph::GraphSemantics {
        athena_graph::GraphSemantics {
            direction: self.direction,
            multiplicity: if self.allows_parallel_edges { MultiplicityPolicy::Multi } else { MultiplicityPolicy::Simple },
            allows_self_loops: self.allows_self_loops,
            self_loop_degree: SelfLoopDegree::One,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphAssumptions {
    /// 附着假设集 id（0 = 无条件）。
    pub assumption_set_id: u64,
}

/// 构造来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphProvenance {
    /// 调用方直接构造。
    Explicit,
    /// 自邻接表导入。
    FromEdgeList,
}

/// 内存图载体（引导实现）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct MemoryGraph {
    /// 语义。
    pub semantics: GraphDomainSemantics,
    /// 边 `(source, target, weight)`；无权时 weight 忽略（算法按 1 计）。
    pub edges: Vec<(GraphNodeId, GraphNodeId, u64)>,
}

impl MemoryGraph {
    /// 节点数（由边推断的最小上界 + 1，或显式给定）。
    pub fn node_count(&self) -> u64 {
        self.edges.iter().flat_map(|(s, t, _)| [s.0, t.0]).max().map(|m| m + 1).unwrap_or(0)
    }

    /// 物化为 [`athena_graph::ImmutableGraph`]（保留句柄对应的 [`GraphId`]）。
    pub fn to_athena_graph(&self, graph_id: GraphId, node_count: u64) -> athena_graph::ImmutableGraph<(), u64> {
        let n = node_count.max(self.node_count());
        let mut builder = GraphBuilder::with_id(graph_id, self.semantics.to_structural());
        builder.transaction(|g| {
            for _ in 0..n {
                g.add_node(());
            }
            for (s, t, w) in &self.edges {
                let _ = g.add_edge(*s, *t, *w);
            }
        });
        builder.finish()
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { semantics: self.semantics, edges: self.edges.clone() }
    }
}

/// 图论领域对象（绑定 [`GraphSnapshot`]）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct GraphObject {
    /// 句柄（与 `snapshot.graph_id` 对齐）。
    pub handle: GraphHandle,
    /// 算法可绑定的不可变观测。
    pub snapshot: GraphSnapshot,
    /// 语义（含权重域）。
    pub semantics: GraphDomainSemantics,
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
    /// 逻辑节点数。
    pub fn node_count(&self) -> u64 {
        self.handle.node_count.max(self.memory.node_count())
    }

    /// 物化为 [`athena_graph::ImmutableGraph`]。
    pub fn to_athena_graph(&self) -> athena_graph::ImmutableGraph<(), u64> {
        self.memory.to_athena_graph(self.snapshot.graph_id, self.handle.node_count)
    }

    /// 从边列表构造（节点 id 须从 0 连续或稀疏，由 `node_count` 指定上界）。
    ///
    /// 经 [`GraphBuilder`] 物化以得到真实 revision，并写入 [`GraphSnapshot`]。
    pub fn from_edges(handle: GraphHandle, semantics: GraphDomainSemantics, edges: Vec<(GraphNodeId, GraphNodeId, u64)>) -> Self {
        let memory = MemoryGraph { semantics, edges };
        let frozen = {
            let mut builder = GraphBuilder::with_id(handle.graph_id(), semantics.to_structural());
            builder.graph_mut().transaction(|g| {
                for _ in 0..handle.node_count.max(memory.node_count()) {
                    g.add_node(());
                }
                for (s, t, w) in &memory.edges {
                    let _ = g.add_edge(*s, *t, *w);
                }
            });
            builder.finish()
        };
        let snapshot = GraphSnapshot::new(frozen.id(), frozen.revision(), semantics.to_structural(), RepresentationId::ADJACENCY_LIST);
        debug_assert_eq!(snapshot.graph_id, handle.graph_id());
        Self {
            handle,
            snapshot,
            semantics,
            presentation: GraphPresentation::AdjacencyList,
            assumptions: GraphAssumptions::default(),
            provenance: GraphProvenance::FromEdgeList,
            memory,
        }
    }

    /// 快照修订号。
    pub const fn revision(&self) -> GraphRevision {
        self.snapshot.revision
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            handle: self.handle,
            snapshot: self.snapshot,
            semantics: self.semantics,
            presentation: self.presentation,
            assumptions: self.assumptions,
            provenance: self.provenance,
            memory: self.memory.owning_copy(),
        }
    }
}
