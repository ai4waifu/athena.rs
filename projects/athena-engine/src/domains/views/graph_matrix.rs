//! [`GraphMatrixView`] — 图 → 稀疏邻接投影（零拷贝脚手架）。
//!
//! 图语义 ≠ 矩阵 CSR 伪装：本视图只借用边表，不物化独立 `MatrixValue`。

use super::{TypedViewHeader, ViewFingerprint, ViewKind, ViewRevision};
use crate::{
    domains::graph_theory::{GraphNodeId, GraphObject},
    reasoning::mgraph::{ObjectRef, TheoryContextId},
};

/// 只读图邻接投影：借用 [`GraphObject`] 边表，禁止拷成 CSR DomainObject。
#[derive(Debug, Clone, Copy)]
pub struct GraphMatrixView<'a> {
    header: TypedViewHeader,
    graph: &'a GraphObject,
}

impl<'a> GraphMatrixView<'a> {
    /// 打开图矩阵风格视图（session-local provisional fingerprint）。
    pub fn open(graph: &'a GraphObject) -> Self {
        let fingerprint = ViewFingerprint(provisional_graph_view_fingerprint(graph));
        let source = ObjectRef::new(TheoryContextId::GRAPH, fingerprint.0);
        let header = TypedViewHeader::new(source, ViewKind::GraphMatrix, ViewRevision(graph.revision().0), fingerprint);
        Self { header, graph }
    }

    /// 公共头。
    pub const fn header(&self) -> TypedViewHeader {
        self.header
    }

    /// 逻辑节点数。
    pub fn node_count(&self) -> u64 {
        self.graph.node_count()
    }

    /// 边表（借用；`(source, target, weight)`）。
    pub fn edges(&self) -> &'a [(GraphNodeId, GraphNodeId, u64)] {
        self.graph.memory.edges.as_slice()
    }

    /// 边数（稀疏 nnz）。
    pub fn nnz(&self) -> usize {
        self.graph.memory.edges.len()
    }
}

fn provisional_graph_view_fingerprint(graph: &GraphObject) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for byte in b"AGV0" {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for byte in graph.handle.id.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for byte in graph.revision().0.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
