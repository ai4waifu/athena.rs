//! 零拷贝逻辑图视图：反向 · 诱导子图 · 边过滤（含 base mapping 与 stale 校验）。

use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use crate::{
    EdgeId, GraphDirection, GraphError, GraphId, GraphRevision, GraphView, MutableGraph, NodeId, SourceEdgeRef, SourceNodeRef,
    ViewEdgeRef, ViewFingerprint, ViewMapping, ViewNodeRef, ViewTransform,
};

fn hash_nodes(nodes: &HashSet<u64>) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut v: Vec<u64> = nodes.iter().copied().collect();
    v.sort_unstable();
    v.hash(&mut hasher);
    hasher.finish()
}

fn ensure_view_ref(fingerprint: ViewFingerprint, view_ref: ViewFingerprint) -> Result<(), GraphError> {
    if fingerprint != view_ref {
        return Err(GraphError::WrongView { expected: view_ref, actual: fingerprint });
    }
    Ok(())
}

/// 有向图反向视图（沿原图入边前进）。
#[derive(Debug, Clone)]
pub struct ReversedGraphView<'a, N, E> {
    graph: &'a MutableGraph<N, E>,
    mapping: ViewMapping,
}

impl<'a, N, E> ReversedGraphView<'a, N, E> {
    /// 包装有向图。
    pub fn new(graph: &'a MutableGraph<N, E>) -> Option<Self> {
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

    /// 视图指纹。
    pub fn view_fingerprint(&self) -> ViewFingerprint {
        self.mapping.view_fingerprint
    }

    /// 底图 revision 仍有效。
    pub fn ensure_fresh(&self) -> Result<(), GraphError> {
        self.mapping.ensure_fresh(self.graph.id(), self.graph.revision())
    }

    /// 绑定 [`ViewNodeRef`]。
    pub fn view_node_ref(&self, node: NodeId) -> Result<ViewNodeRef, GraphError> {
        self.ensure_fresh()?;
        if node.0 >= self.graph.node_count() {
            return Err(GraphError::InvalidNode);
        }
        Ok(ViewNodeRef::new(self.mapping.view_fingerprint, node))
    }

    /// 绑定 [`ViewEdgeRef`]。
    pub fn view_edge_ref(&self, edge: EdgeId) -> Result<ViewEdgeRef, GraphError> {
        self.ensure_fresh()?;
        if edge.0 >= self.graph.edge_count() {
            return Err(GraphError::InvalidNode);
        }
        Ok(ViewEdgeRef::new(self.mapping.view_fingerprint, edge))
    }

    /// [`ViewNodeRef`] → [`SourceNodeRef`]（反向视图：local id 恒等）。
    pub fn map_view_node_to_source(&self, view_ref: ViewNodeRef) -> Result<SourceNodeRef, GraphError> {
        self.ensure_fresh()?;
        ensure_view_ref(self.mapping.view_fingerprint, view_ref.view)?;
        let base = self.map_node_to_base(view_ref.node)?.ok_or(GraphError::InvalidNode)?;
        Ok(SourceNodeRef::new(self.mapping.base_graph_id, self.mapping.base_revision, base))
    }

    /// [`ViewEdgeRef`] → [`SourceEdgeRef`]。
    pub fn map_view_edge_to_source(&self, view_ref: ViewEdgeRef) -> Result<SourceEdgeRef, GraphError> {
        self.ensure_fresh()?;
        ensure_view_ref(self.mapping.view_fingerprint, view_ref.view)?;
        let base = self.map_edge_to_base(view_ref.edge)?.ok_or(GraphError::InvalidNode)?;
        Ok(SourceEdgeRef::new(self.mapping.base_graph_id, self.mapping.base_revision, base))
    }

    /// view 节点 → base 节点（恒等）；视图过期则 `Err`。
    pub fn map_node_to_base(&self, node: NodeId) -> Result<Option<NodeId>, GraphError> {
        self.ensure_fresh()?;
        Ok((node.0 < self.graph.node_count()).then_some(node))
    }

    /// view 边 → base 边（恒等；定向在呈现层翻转）。
    pub fn map_edge_to_base(&self, edge: EdgeId) -> Result<Option<EdgeId>, GraphError> {
        self.ensure_fresh()?;
        Ok((edge.0 < self.graph.edge_count()).then_some(edge))
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
    pub fn node_count(&self) -> Result<u64, GraphError> {
        self.ensure_fresh()?;
        Ok(self.graph.node_count())
    }

    /// 反向后的出邻接 = 原图入邻接。
    pub fn neighbors(&self, node: NodeId) -> Result<impl Iterator<Item = NodeId> + '_, GraphError> {
        self.ensure_fresh()?;
        Ok(self.graph.in_neighbors(node).into_iter())
    }
}

/// 诱导子图视图（仅保留给定节点及其内部边）。
#[derive(Debug, Clone)]
pub struct InducedSubgraphView<'a, N, E> {
    graph: &'a MutableGraph<N, E>,
    nodes: HashSet<u64>,
    mapping: ViewMapping,
}

impl<'a, N, E> InducedSubgraphView<'a, N, E> {
    /// 构造诱导子图；空集表示空图投影。节点 id 仍为 base local id。
    pub fn new(graph: &'a MutableGraph<N, E>, keep: impl IntoIterator<Item = NodeId>) -> Self {
        let nodes: HashSet<u64> = keep.into_iter().map(|n| n.0).collect();
        let transform_hash = hash_nodes(&nodes);
        let mapping = ViewMapping::new(graph.id(), graph.revision(), ViewTransform::Induced, transform_hash);
        Self { graph, nodes, mapping }
    }

    /// 底图映射合同。
    pub fn mapping(&self) -> &ViewMapping {
        &self.mapping
    }

    /// 视图指纹。
    pub fn view_fingerprint(&self) -> ViewFingerprint {
        self.mapping.view_fingerprint
    }

    /// 底图 revision 仍有效。
    pub fn ensure_fresh(&self) -> Result<(), GraphError> {
        self.mapping.ensure_fresh(self.graph.id(), self.graph.revision())
    }

    /// 绑定 [`ViewNodeRef`]。
    pub fn view_node_ref(&self, node: NodeId) -> Result<ViewNodeRef, GraphError> {
        self.ensure_fresh()?;
        if !self.nodes.contains(&node.0) {
            return Err(GraphError::InvalidNode);
        }
        Ok(ViewNodeRef::new(self.mapping.view_fingerprint, node))
    }

    /// 绑定 [`ViewEdgeRef`]（两端均在保留集内）。
    pub fn view_edge_ref(&self, edge: EdgeId) -> Result<ViewEdgeRef, GraphError> {
        self.ensure_fresh()?;
        let _ = self.map_edge_to_base(edge)?.ok_or(GraphError::InvalidNode)?;
        Ok(ViewEdgeRef::new(self.mapping.view_fingerprint, edge))
    }

    /// [`ViewNodeRef`] → [`SourceNodeRef`]。
    pub fn map_view_node_to_source(&self, view_ref: ViewNodeRef) -> Result<SourceNodeRef, GraphError> {
        self.ensure_fresh()?;
        ensure_view_ref(self.mapping.view_fingerprint, view_ref.view)?;
        let base = self.map_node_to_base(view_ref.node)?.ok_or(GraphError::InvalidNode)?;
        Ok(SourceNodeRef::new(self.mapping.base_graph_id, self.mapping.base_revision, base))
    }

    /// [`ViewEdgeRef`] → [`SourceEdgeRef`]。
    pub fn map_view_edge_to_source(&self, view_ref: ViewEdgeRef) -> Result<SourceEdgeRef, GraphError> {
        self.ensure_fresh()?;
        ensure_view_ref(self.mapping.view_fingerprint, view_ref.view)?;
        let base = self.map_edge_to_base(view_ref.edge)?.ok_or(GraphError::InvalidNode)?;
        Ok(SourceEdgeRef::new(self.mapping.base_graph_id, self.mapping.base_revision, base))
    }

    /// view 节点 → base（恒等过滤）。
    pub fn map_node_to_base(&self, node: NodeId) -> Result<Option<NodeId>, GraphError> {
        self.ensure_fresh()?;
        Ok(self.contains_unchecked(node).then_some(node))
    }

    /// view 边 → base：两端均在保留集内时返回同一 `EdgeId`。
    pub fn map_edge_to_base(&self, edge: EdgeId) -> Result<Option<EdgeId>, GraphError> {
        self.ensure_fresh()?;
        let Some((s, t)) = self.graph.edge_endpoints(edge)
        else {
            return Ok(None);
        };
        Ok((self.contains_unchecked(s) && self.contains_unchecked(t)).then_some(edge))
    }

    /// 节点是否保留（需 fresh）。
    pub fn contains(&self, node: NodeId) -> Result<bool, GraphError> {
        self.ensure_fresh()?;
        Ok(self.contains_unchecked(node))
    }

    fn contains_unchecked(&self, node: NodeId) -> bool {
        self.nodes.contains(&node.0)
    }

    /// 保留节点数。
    pub fn node_count(&self) -> Result<u64, GraphError> {
        self.ensure_fresh()?;
        Ok(self.nodes.len() as u64)
    }

    /// 方向与底图一致。
    pub fn direction(&self) -> Result<GraphDirection, GraphError> {
        self.ensure_fresh()?;
        Ok(self.graph.direction())
    }

    /// 诱导子图上的邻接（两端均在保留集内）。
    pub fn neighbors(&self, node: NodeId) -> Result<impl Iterator<Item = NodeId> + '_, GraphError> {
        self.ensure_fresh()?;
        let keep = &self.nodes;
        Ok(self.graph.out_neighbors(node).filter(move |target| keep.contains(&node.0) && keep.contains(&target.0)))
    }
}

/// 边过滤视图（保留满足谓词的边）。
#[derive(Debug, Clone)]
pub struct EdgeFilteredView<'a, N, E, F> {
    graph: &'a MutableGraph<N, E>,
    predicate: F,
    mapping: ViewMapping,
}

impl<'a, N, E, F> EdgeFilteredView<'a, N, E, F>
where
    F: Copy + Fn(NodeId, NodeId, EdgeId) -> bool,
{
    /// 包装图引用与边谓词。
    pub fn new(graph: &'a MutableGraph<N, E>, predicate: F) -> Self {
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

    /// 视图指纹。
    pub fn view_fingerprint(&self) -> ViewFingerprint {
        self.mapping.view_fingerprint
    }

    /// 底图 revision 仍有效。
    pub fn ensure_fresh(&self) -> Result<(), GraphError> {
        self.mapping.ensure_fresh(self.graph.id(), self.graph.revision())
    }

    /// 绑定 [`ViewNodeRef`]。
    pub fn view_node_ref(&self, node: NodeId) -> Result<ViewNodeRef, GraphError> {
        self.ensure_fresh()?;
        if node.0 >= self.graph.node_count() {
            return Err(GraphError::InvalidNode);
        }
        Ok(ViewNodeRef::new(self.mapping.view_fingerprint, node))
    }

    /// 绑定 [`ViewEdgeRef`]（谓词须接受该边）。
    pub fn view_edge_ref(&self, edge: EdgeId) -> Result<ViewEdgeRef, GraphError> {
        self.ensure_fresh()?;
        let _ = self.map_edge_to_base(edge)?.ok_or(GraphError::InvalidNode)?;
        Ok(ViewEdgeRef::new(self.mapping.view_fingerprint, edge))
    }

    /// [`ViewNodeRef`] → [`SourceNodeRef`]。
    pub fn map_view_node_to_source(&self, view_ref: ViewNodeRef) -> Result<SourceNodeRef, GraphError> {
        self.ensure_fresh()?;
        ensure_view_ref(self.mapping.view_fingerprint, view_ref.view)?;
        let base = self.map_node_to_base(view_ref.node)?.ok_or(GraphError::InvalidNode)?;
        Ok(SourceNodeRef::new(self.mapping.base_graph_id, self.mapping.base_revision, base))
    }

    /// [`ViewEdgeRef`] → [`SourceEdgeRef`]。
    pub fn map_view_edge_to_source(&self, view_ref: ViewEdgeRef) -> Result<SourceEdgeRef, GraphError> {
        self.ensure_fresh()?;
        ensure_view_ref(self.mapping.view_fingerprint, view_ref.view)?;
        let base = self.map_edge_to_base(view_ref.edge)?.ok_or(GraphError::InvalidNode)?;
        Ok(SourceEdgeRef::new(self.mapping.base_graph_id, self.mapping.base_revision, base))
    }

    /// view 节点 → base（恒等）。
    pub fn map_node_to_base(&self, node: NodeId) -> Result<Option<NodeId>, GraphError> {
        self.ensure_fresh()?;
        Ok((node.0 < self.graph.node_count()).then_some(node))
    }

    /// 仅当谓词接受该边时映回 base。
    pub fn map_edge_to_base(&self, edge: EdgeId) -> Result<Option<EdgeId>, GraphError> {
        self.ensure_fresh()?;
        let Some((s, t)) = self.graph.edge_endpoints(edge)
        else {
            return Ok(None);
        };
        Ok((self.predicate)(s, t, edge).then_some(edge))
    }

    /// 过滤后的邻接（沿保留边前进）。
    pub fn neighbors(&self, node: NodeId) -> Result<impl Iterator<Item = NodeId> + '_, GraphError> {
        self.ensure_fresh()?;
        Ok(self
            .graph
            .out_neighbors(node)
            .filter(move |&target| self.graph.find_edge(node, target).is_some_and(|edge| (self.predicate)(node, target, edge))))
    }

    /// 节点数（与底图相同）。
    pub fn node_count(&self) -> Result<u64, GraphError> {
        self.ensure_fresh()?;
        Ok(self.graph.node_count())
    }
}
