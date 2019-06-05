//! 图结构语义、快照与内容指纹。

use super::{
    direction::GraphDirection,
    id::{GraphId, GraphRevision, RepresentationId},
};

/// 平行边策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MultiplicityPolicy {
    /// 简单图：拒绝平行边（引导实现仍允许存储层插入时由调用方约束）。
    #[default]
    Simple,
    /// 允许多重边。
    Multi,
}

/// 自环对 degree 的贡献（邻接登记次数可与此独立；须文档一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelfLoopDegree {
    /// 自环对 degree 贡献 1（与当前无向邻接只登记一次一致）。
    One,
    /// 自环对 degree 贡献 2（incidence 惯例）。
    Two,
}

impl Default for SelfLoopDegree {
    fn default() -> Self {
        Self::One
    }
}

/// 结构层图语义（方向 · 多重边 · 自环 · degree 约定）。
///
/// 权重域属于 `graph_theory`，不放在本结构中。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphSemantics {
    /// 有向 / 无向。
    pub direction: GraphDirection,
    /// 平行边策略。
    pub multiplicity: MultiplicityPolicy,
    /// 是否允许自环。
    pub allows_self_loops: bool,
    /// 自环对 degree 的贡献。
    pub self_loop_degree: SelfLoopDegree,
}

impl GraphSemantics {
    /// 有向简单图默认语义。
    pub const fn directed_simple() -> Self {
        Self {
            direction: GraphDirection::Directed,
            multiplicity: MultiplicityPolicy::Simple,
            allows_self_loops: false,
            self_loop_degree: SelfLoopDegree::One,
        }
    }

    /// 无向简单图默认语义。
    pub const fn undirected_simple() -> Self {
        Self {
            direction: GraphDirection::Undirected,
            multiplicity: MultiplicityPolicy::Simple,
            allows_self_loops: false,
            self_loop_degree: SelfLoopDegree::One,
        }
    }

    /// 由方向构造默认简单图语义。
    pub const fn from_direction(direction: GraphDirection) -> Self {
        match direction {
            GraphDirection::Directed => Self::directed_simple(),
            GraphDirection::Undirected => Self::undirected_simple(),
        }
    }
}

impl Default for GraphSemantics {
    fn default() -> Self {
        Self::directed_simple()
    }
}

/// 算法可绑定的不可变图观测。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphSnapshot {
    /// 逻辑图身份。
    pub graph_id: GraphId,
    /// 观测时的修订号。
    pub revision: GraphRevision,
    /// 结构语义。
    pub semantics: GraphSemantics,
    /// 物理表示身份。
    pub representation: RepresentationId,
}

impl GraphSnapshot {
    /// 构造快照。
    pub const fn new(graph_id: GraphId, revision: GraphRevision, semantics: GraphSemantics, representation: RepresentationId) -> Self {
        Self { graph_id, revision, semantics, representation }
    }
}

/// 内容/语义指纹（跨 session 筛选用；**不是** canonical mathematical identity）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphFingerprint {
    /// 节点数。
    pub node_count: u64,
    /// 边数。
    pub edge_count: u64,
    /// 结构语义。
    pub semantics: GraphSemantics,
    /// 边列表结构 hash（非同构证明）。
    pub structure_hash: u64,
}

/// 视图变换种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewTransform {
    /// 有向图反向呈现。
    Reversed,
    /// 诱导子图（节点子集）。
    Induced,
    /// 边过滤。
    EdgeFiltered,
}

/// 视图身份指纹。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewFingerprint {
    /// 底图身份。
    pub base_graph_id: GraphId,
    /// 底图修订。
    pub base_revision: GraphRevision,
    /// 变换种类。
    pub transform: ViewTransform,
    /// 变换参数 hash（如保留节点集）。
    pub transform_hash: u64,
}

/// 视图 ↔ 底图映射合同（节点/边在 view 中仍使用 base 的 local id 时，映射为恒等过滤）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewMapping {
    /// 底图身份。
    pub base_graph_id: GraphId,
    /// 底图修订。
    pub base_revision: GraphRevision,
    /// 视图指纹。
    pub view_fingerprint: ViewFingerprint,
    /// 变换。
    pub transform: ViewTransform,
}

impl ViewMapping {
    /// 构造。
    pub fn new(base_graph_id: GraphId, base_revision: GraphRevision, transform: ViewTransform, transform_hash: u64) -> Self {
        Self {
            base_graph_id,
            base_revision,
            view_fingerprint: ViewFingerprint { base_graph_id, base_revision, transform, transform_hash },
            transform,
        }
    }

    /// 校验底图身份与 revision 仍匹配；过期返回 [`crate::GraphError::StaleView`]。
    pub fn ensure_fresh(&self, graph_id: GraphId, revision: GraphRevision) -> Result<(), crate::GraphError> {
        if self.base_graph_id != graph_id {
            return Err(crate::GraphError::WrongGraph { expected: self.base_graph_id, actual: graph_id });
        }
        if self.base_revision != revision {
            return Err(crate::GraphError::StaleView { expected: self.base_revision, actual: revision });
        }
        Ok(())
    }
}

/// CSR/CSC 等存储元数据（使表示可回溯到逻辑图状态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphStorageMetadata {
    /// 表示身份。
    pub representation_id: RepresentationId,
    /// 可选逻辑图身份。
    pub graph_id: Option<GraphId>,
    /// 可选修订。
    pub revision: Option<GraphRevision>,
    /// 可选语义。
    pub semantics: Option<GraphSemantics>,
    /// 是否声称邻接已排序。
    pub sorted_adjacency: bool,
    /// 是否允许同一出边重复 target。
    pub allows_duplicate_targets: bool,
}

impl GraphStorageMetadata {
    /// CSR 默认元数据（无绑定逻辑图）。
    pub fn csr_unbound(sorted_adjacency: bool) -> Self {
        Self {
            representation_id: RepresentationId::CSR,
            graph_id: None,
            revision: None,
            semantics: None,
            sorted_adjacency,
            allows_duplicate_targets: false,
        }
    }

    /// 绑定到快照。
    pub fn bind_snapshot(mut self, snapshot: GraphSnapshot) -> Self {
        self.graph_id = Some(snapshot.graph_id);
        self.revision = Some(snapshot.revision);
        self.semantics = Some(snapshot.semantics);
        self.representation_id = snapshot.representation;
        self
    }
}
