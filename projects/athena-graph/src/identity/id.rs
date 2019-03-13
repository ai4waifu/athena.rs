//! 节点 / 边 / 图身份与修订号。

use std::sync::atomic::{AtomicU64, Ordering};

/// Session/local 逻辑图身份（非内容指纹）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct GraphId(pub u64);

static NEXT_GRAPH_ID: AtomicU64 = AtomicU64::new(1);

impl GraphId {
    /// 分配新的 session-local 图身份。
    pub fn allocate() -> Self {
        Self(NEXT_GRAPH_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// 由调用方（Session）指定原始 id；不推进分配器。
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

/// 物理表示身份（CSR/CSC/邻接表等，非数学图身份）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RepresentationId(pub u64);

impl RepresentationId {
    /// 内存邻接表表示。
    pub const ADJACENCY_LIST: Self = Self(1);
    /// CSR 表示。
    pub const CSR: Self = Self(2);
    /// CSC 表示。
    pub const CSC: Self = Self(3);
}

/// 图结构修订号。
///
/// 图生命周期：
/// - `add_node` / `add_edge` 各递增 1
/// - [`super::Graph::transaction`] 内多次 mutation 在提交时只递增 1
/// - 视图创建不递增底图 revision
/// - 溢出时 saturating（`u64::MAX` 后保持）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub struct GraphRevision(pub u64);

impl GraphRevision {
    /// 单调递增（饱和）。
    pub fn bump(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// 存储 wire 节点身份（`u64` newtype）；仅在单图某一 revision 内有意义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeId(pub u64);

/// 存储 wire 边身份（`u64` newtype）；仅在单图某一 revision 内有意义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct EdgeId(pub u64);

/// 跨对象节点引用（绑定 [`GraphId`] + [`GraphRevision`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeRef {
    /// 所属逻辑图。
    pub graph_id: GraphId,
    /// 绑定的结构修订。
    pub revision: GraphRevision,
    /// 图内节点 id。
    pub node: NodeId,
}

/// 跨对象边引用（绑定 [`GraphId`] + [`GraphRevision`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct EdgeRef {
    /// 所属逻辑图。
    pub graph_id: GraphId,
    /// 绑定的结构修订。
    pub revision: GraphRevision,
    /// 图内边 id。
    pub edge: EdgeId,
}

impl NodeRef {
    /// 构造引用。
    pub const fn new(graph_id: GraphId, revision: GraphRevision, node: NodeId) -> Self {
        Self { graph_id, revision, node }
    }
}

impl EdgeRef {
    /// 构造引用。
    pub const fn new(graph_id: GraphId, revision: GraphRevision, edge: EdgeId) -> Self {
        Self { graph_id, revision, edge }
    }
}

use super::semantics::ViewFingerprint;

/// 底图节点引用（长期身份；禁止用裸 [`NodeId`] 跨视图保存）。
pub type SourceNodeRef = NodeRef;

/// 底图边引用。
pub type SourceEdgeRef = EdgeRef;

/// 视图空间节点引用（绑 [`ViewFingerprint`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewNodeRef {
    /// 视图指纹（含底图 id / revision / 变换）。
    pub view: ViewFingerprint,
    /// 视图内 local 节点 id（诱导/过滤当前与 base local id 同形）。
    pub node: NodeId,
}

/// 视图空间边引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewEdgeRef {
    /// 视图指纹。
    pub view: ViewFingerprint,
    /// 视图内 local 边 id。
    pub edge: EdgeId,
}

impl ViewNodeRef {
    /// 构造。
    pub const fn new(view: ViewFingerprint, node: NodeId) -> Self {
        Self { view, node }
    }
}

impl ViewEdgeRef {
    /// 构造。
    pub const fn new(view: ViewFingerprint, edge: EdgeId) -> Self {
        Self { view, edge }
    }
}
