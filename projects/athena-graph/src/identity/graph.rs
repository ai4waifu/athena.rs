//! 内存邻接图、Builder、不可变图与 capability。

use super::{
    direction::GraphDirection,
    id::{EdgeId, EdgeRef, GraphId, GraphRevision, NodeId, NodeRef, RepresentationId},
    semantics::{GraphFingerprint, GraphSemantics, GraphSnapshot},
};
use crate::{GraphAlgorithmRequirements, GraphCapabilities, GraphError};

/// 构造期可变邻接表（便利实现，不是规模上限）。
///
/// 公开稳定路径：[`GraphBuilder`] → [`ImmutableGraph`]。本类型的构造函数为 `pub(crate)`，
/// 外部 crate 只能经 builder 的 [`GraphBuilder::graph_mut`] 在构造期写入。
#[derive(Debug, Clone)]
pub struct MutableGraph<N, E> {
    id: GraphId,
    semantics: GraphSemantics,
    revision: GraphRevision,
    nodes: Vec<N>,
    edges: Vec<(NodeId, NodeId, E)>,
    outgoing: Vec<Vec<EdgeId>>,
    incoming: Vec<Vec<EdgeId>>,
    /// 事务嵌套深度；>0 时 mutation 不立即 bump revision。
    txn_depth: u32,
    /// 当前事务内是否发生过 mutation。
    txn_dirty: bool,
}

impl<N, E> MutableGraph<N, E> {
    /// 创建空图（分配新 [`GraphId`]，默认简单图语义）。
    pub(crate) fn new(direction: GraphDirection) -> Self {
        Self::with_semantics(GraphSemantics::from_direction(direction))
    }

    /// 以给定语义创建空图（分配新 [`GraphId`]）。
    pub(crate) fn with_semantics(semantics: GraphSemantics) -> Self {
        Self::with_id(GraphId::allocate(), semantics)
    }

    /// 以指定 [`GraphId`] 与语义创建空图。
    pub(crate) fn with_id(id: GraphId, semantics: GraphSemantics) -> Self {
        Self {
            id,
            semantics,
            revision: GraphRevision(0),
            nodes: Vec::new(),
            edges: Vec::new(),
            outgoing: Vec::new(),
            incoming: Vec::new(),
            txn_depth: 0,
            txn_dirty: false,
        }
    }

    /// 逻辑图身份。
    pub const fn id(&self) -> GraphId {
        self.id
    }

    /// 结构语义。
    pub const fn semantics(&self) -> GraphSemantics {
        self.semantics
    }

    /// 方向。
    pub const fn direction(&self) -> GraphDirection {
        self.semantics.direction
    }

    /// 结构修订号。
    pub const fn revision(&self) -> GraphRevision {
        self.revision
    }

    /// 当前邻接表快照。
    pub fn snapshot(&self) -> GraphSnapshot {
        GraphSnapshot::new(self.id, self.revision, self.semantics, RepresentationId::ADJACENCY_LIST)
    }

    /// 内容指纹（非 canonical identity）。
    pub fn fingerprint(&self) -> GraphFingerprint
    where
        E: std::hash::Hash,
    {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };
        let mut hasher = DefaultHasher::new();
        self.id.0.hash(&mut hasher);
        self.semantics.hash(&mut hasher);
        for (s, t, e) in &self.edges {
            s.0.hash(&mut hasher);
            t.0.hash(&mut hasher);
            e.hash(&mut hasher);
        }
        GraphFingerprint {
            node_count: self.node_count(),
            edge_count: self.edge_count(),
            semantics: self.semantics,
            structure_hash: hasher.finish(),
        }
    }

    /// 绑定当前 revision 的 [`NodeRef`]。
    pub const fn node_ref(&self, node: NodeId) -> NodeRef {
        NodeRef::new(self.id, self.revision, node)
    }

    /// 绑定当前 revision 的 [`EdgeRef`]。
    pub const fn edge_ref(&self, edge: EdgeId) -> EdgeRef {
        EdgeRef::new(self.id, self.revision, edge)
    }

    /// 校验 [`NodeRef`] 并返回 local [`NodeId`]。
    pub fn resolve_node_ref(&self, node_ref: NodeRef) -> Result<NodeId, GraphError> {
        if node_ref.graph_id != self.id {
            return Err(GraphError::WrongGraph { expected: node_ref.graph_id, actual: self.id });
        }
        if node_ref.revision != self.revision {
            return Err(GraphError::StaleRef { expected: node_ref.revision, actual: self.revision });
        }
        if node_ref.node.0 >= self.node_count() {
            return Err(GraphError::InvalidNode);
        }
        Ok(node_ref.node)
    }

    /// 校验 [`EdgeRef`] 并返回 local [`EdgeId`]。
    pub fn resolve_edge_ref(&self, edge_ref: EdgeRef) -> Result<EdgeId, GraphError> {
        if edge_ref.graph_id != self.id {
            return Err(GraphError::WrongGraph { expected: edge_ref.graph_id, actual: self.id });
        }
        if edge_ref.revision != self.revision {
            return Err(GraphError::StaleRef { expected: edge_ref.revision, actual: self.revision });
        }
        if edge_ref.edge.0 >= self.edge_count() {
            return Err(GraphError::InvalidNode);
        }
        Ok(edge_ref.edge)
    }

    fn bump_revision(&mut self) {
        if self.txn_depth > 0 {
            self.txn_dirty = true;
        }
        else {
            self.revision = self.revision.bump();
        }
    }

    /// 事务：多次 mutation 在闭包结束后只递增一次 revision（若有变更）。
    pub fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.txn_depth = self.txn_depth.saturating_add(1);
        let result = f(self);
        self.txn_depth = self.txn_depth.saturating_sub(1);
        if self.txn_depth == 0 && self.txn_dirty {
            self.revision = self.revision.bump();
            self.txn_dirty = false;
        }
        result
    }

    /// 添加节点。
    pub fn add_node(&mut self, value: N) -> NodeId {
        let id = NodeId(self.nodes.len() as u64);
        self.nodes.push(value);
        self.outgoing.push(Vec::new());
        self.incoming.push(Vec::new());
        self.bump_revision();
        id
    }

    /// 添加边；无向图同时登记反向邻接索引。
    ///
    /// 自环在无向图中只登记一次（与 [`crate::SelfLoopDegree::One`] 默认一致）。
    pub fn add_edge(&mut self, source: NodeId, target: NodeId, value: E) -> Option<EdgeId> {
        let n = self.nodes.len() as u64;
        if source.0 >= n || target.0 >= n {
            return None;
        }
        if source == target && !self.semantics.allows_self_loops {
            return None;
        }
        let id = EdgeId(self.edges.len() as u64);
        self.edges.push((source, target, value));
        self.outgoing[source.0 as usize].push(id);
        if self.direction() == GraphDirection::Directed {
            self.incoming[target.0 as usize].push(id);
        }
        else if source != target {
            self.outgoing[target.0 as usize].push(id);
        }
        self.bump_revision();
        Some(id)
    }

    /// 出邻接目标（有向=出边；无向=邻接）。
    pub fn out_neighbors(&self, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.outgoing.get(node.0 as usize).into_iter().flatten().map(move |edge| self.target_of_edge(*edge, node))
    }

    /// 入邻接列表（有向=入边源；无向=邻接）。
    pub fn in_neighbors(&self, node: NodeId) -> Vec<NodeId> {
        if self.direction() == GraphDirection::Undirected {
            self.out_neighbors(node).collect()
        }
        else {
            self.incoming.get(node.0 as usize).map(|list| list.iter().map(|edge| self.source_of_edge(*edge)).collect()).unwrap_or_default()
        }
    }

    /// 邻接迭代（有向图取出发邻接；无向图取无向邻接）。
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

    /// 边端点。
    pub fn edge_endpoints(&self, edge: EdgeId) -> Option<(NodeId, NodeId)> {
        self.edges.get(edge.0 as usize).map(|(s, t, _)| (*s, *t))
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

    /// 冻结为不可变图（移动所有权）。
    pub fn into_immutable(self) -> ImmutableGraph<N, E> {
        ImmutableGraph { inner: self }
    }

    /// 本表示的 capability 报告。
    pub fn capabilities(&self) -> GraphCapabilities {
        GraphCapabilities {
            in_memory: true,
            sorted_adjacency: false,
            reverse_adjacency: self.direction() == GraphDirection::Directed,
            random_access: true,
            chunked_sequential: false,
            external_workspace: false,
            distributed_shards: false,
        }
    }

    /// 校验是否满足算法需求。
    pub fn ensure_capabilities(&self, req: GraphAlgorithmRequirements) -> Result<(), crate::GraphError> {
        if self.capabilities().satisfies(req) { Ok(()) } else { Err(crate::GraphError::CapabilityMismatch { requirement: req }) }
    }

    fn target_of_edge(&self, edge: EdgeId, from: NodeId) -> NodeId {
        let (s, t, _) = &self.edges[edge.0 as usize];
        if self.direction() == GraphDirection::Undirected && *s == from {
            *t
        }
        else if self.direction() == GraphDirection::Undirected {
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

/// 可变构造器 → [`ImmutableGraph`]（公开稳定构造路径）。
#[derive(Debug)]
pub struct GraphBuilder<N, E> {
    graph: MutableGraph<N, E>,
}

impl<N, E> GraphBuilder<N, E> {
    /// 新构造器（分配 `GraphId`）。
    pub fn new(semantics: GraphSemantics) -> Self {
        Self { graph: MutableGraph::with_semantics(semantics) }
    }

    /// 由方向构造（默认简单图语义）。
    pub fn from_direction(direction: GraphDirection) -> Self {
        Self::new(GraphSemantics::from_direction(direction))
    }

    /// 指定 `GraphId` 的构造器。
    pub fn with_id(id: GraphId, semantics: GraphSemantics) -> Self {
        Self { graph: MutableGraph::with_id(id, semantics) }
    }

    /// 添加节点。
    pub fn add_node(&mut self, value: N) -> NodeId {
        self.graph.add_node(value)
    }

    /// 添加边。
    pub fn add_edge(&mut self, source: NodeId, target: NodeId, value: E) -> Option<EdgeId> {
        self.graph.add_edge(source, target, value)
    }

    /// 构造期事务（多次 mutation 提交时 revision +1）。
    pub fn transaction<R>(&mut self, f: impl FnOnce(&mut MutableGraph<N, E>) -> R) -> R {
        self.graph.transaction(f)
    }

    /// 底层可变图（仅构造期）。
    pub fn graph_mut(&mut self) -> &mut MutableGraph<N, E> {
        &mut self.graph
    }

    /// 只读窥视构造中的图。
    pub fn graph(&self) -> &MutableGraph<N, E> {
        &self.graph
    }

    /// 完成构造，得到不可变图（不登记 GC）。
    pub fn finish(self) -> ImmutableGraph<N, E> {
        ImmutableGraph::from_mutable(self.graph)
    }

    /// 完成构造并在 [`GcHeap`](athena_gc::GcHeap) 上发布 snapshot 根与空 [`ChunkSet`](crate::ChunkSet)。
    pub fn finish_on_heap(self, heap: &mut athena_gc::GcHeap) -> Result<crate::lifecycle::PublishedImmutableGraph<N, E>, GraphError> {
        crate::lifecycle::finish_on_heap(self.graph, heap)
    }

    /// 完成构造：发布 snapshot 根，并将 CSR offsets/indices 写入 GraphIndex segment。
    pub fn finish_csr_on_heap(
        self,
        heap: &mut athena_gc::GcHeap,
        registry: &mut crate::lifecycle::ChunkRegistry,
        budget: athena_ndarray::MemoryBudget,
    ) -> Result<(crate::lifecycle::PublishedImmutableGraph<N, E>, crate::CsrOnHeap), GraphError> {
        crate::storage::finish_csr_on_heap(self.graph, heap, registry, budget)
    }
}

/// 稳定只读图（算法与视图的主路径）。
///
/// 通过 [`Deref`](std::ops::Deref) 暴露 [`MutableGraph`] 的只读 API；不可再 mutation。
#[derive(Debug, Clone)]
pub struct ImmutableGraph<N, E> {
    inner: MutableGraph<N, E>,
}

impl<N, E> ImmutableGraph<N, E> {
    /// 由构造完成的可变图封存（crate 内 / lifecycle 发布路径）。
    pub(crate) fn from_mutable(inner: MutableGraph<N, E>) -> Self {
        Self { inner }
    }

    /// 快照。
    pub fn snapshot(&self) -> GraphSnapshot {
        self.inner.snapshot()
    }

    /// 只读底层存储（与 [`Deref`](std::ops::Deref) 相同）。
    pub fn as_graph(&self) -> &MutableGraph<N, E> {
        &self.inner
    }

    /// 逻辑图身份。
    pub const fn id(&self) -> GraphId {
        self.inner.id()
    }

    /// 修订号。
    pub const fn revision(&self) -> GraphRevision {
        self.inner.revision()
    }

    /// 结构语义。
    pub const fn semantics(&self) -> GraphSemantics {
        self.inner.semantics()
    }

    /// 方向。
    pub const fn direction(&self) -> GraphDirection {
        self.inner.direction()
    }

    /// 绑定 [`NodeRef`]。
    pub const fn node_ref(&self, node: NodeId) -> NodeRef {
        self.inner.node_ref(node)
    }

    /// 绑定 [`EdgeRef`]。
    pub const fn edge_ref(&self, edge: EdgeId) -> EdgeRef {
        self.inner.edge_ref(edge)
    }

    /// 校验 [`NodeRef`]。
    pub fn resolve_node_ref(&self, node_ref: NodeRef) -> Result<NodeId, GraphError> {
        self.inner.resolve_node_ref(node_ref)
    }

    /// 校验 [`EdgeRef`]。
    pub fn resolve_edge_ref(&self, edge_ref: EdgeRef) -> Result<EdgeId, GraphError> {
        self.inner.resolve_edge_ref(edge_ref)
    }

    /// 只读图视图。
    pub fn view(&self) -> GraphView<'_, N, E> {
        self.inner.view()
    }

    /// capability 报告。
    pub fn capabilities(&self) -> GraphCapabilities {
        self.inner.capabilities()
    }

    /// 校验算法需求。
    pub fn ensure_capabilities(&self, req: GraphAlgorithmRequirements) -> Result<(), GraphError> {
        self.inner.ensure_capabilities(req)
    }
}

impl<N, E> std::ops::Deref for ImmutableGraph<N, E> {
    type Target = MutableGraph<N, E>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// 只读图视图（不拥有存储）。
#[derive(Debug, Clone, Copy)]
pub struct GraphView<'a, N, E> {
    graph: &'a MutableGraph<N, E>,
}

impl<'a, N, E> GraphView<'a, N, E> {
    /// 底层图引用（不消费视图）。
    pub const fn graph_ref(&self) -> &'a MutableGraph<N, E> {
        self.graph
    }

    /// 底层图（消费视图）。
    pub const fn graph(self) -> &'a MutableGraph<N, E> {
        self.graph
    }

    /// 逻辑图身份。
    pub const fn id(self) -> GraphId {
        self.graph.id()
    }

    /// 方向。
    pub const fn direction(self) -> GraphDirection {
        self.graph.direction()
    }

    /// 修订号。
    pub const fn revision(self) -> GraphRevision {
        self.graph.revision()
    }

    /// 快照。
    pub fn snapshot(self) -> GraphSnapshot {
        self.graph.snapshot()
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
