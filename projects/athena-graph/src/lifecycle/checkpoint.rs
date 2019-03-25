//! 可恢复算法 checkpoint：绑定 revision + chunk identity。

use crate::{FrontierCheckpoint, GraphId, GraphRevision};

use super::{
    chunk::ChunkSet,
    ids::{GraphChunkId, GraphRevisionId, GraphSnapshotId, GraphWorkspaceId},
};

/// 图算法可恢复检查点（frontier 形状在 graph，策略在 engine）。
///
/// **禁止**内嵌 resident pointer。resume 须重新获取 lease / pin。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAlgorithmCheckpoint {
    /// 稳定观测身份。
    pub snapshot_id: GraphSnapshotId,
    /// 逻辑图。
    pub graph_id: GraphId,
    /// 修订号码（与 [`NodeRef`](crate::NodeRef) 比较同源）。
    pub revision: GraphRevision,
    /// 修订对象身份（Trace / ABA）。
    pub revision_id: GraphRevisionId,
    /// 当时依赖的 chunk 集合。
    pub chunks: ChunkSet,
    /// 算法工作区身份。
    pub workspace_id: GraphWorkspaceId,
    /// Frontier / visited 等算法态（可 spill；不含裸指针）。
    pub frontier: FrontierCheckpoint,
}

impl GraphAlgorithmCheckpoint {
    /// 由 wire 身份与算法态构造。
    pub fn new(
        snapshot_id: GraphSnapshotId,
        graph_id: GraphId,
        revision: GraphRevision,
        revision_id: GraphRevisionId,
        chunk_ids: impl IntoIterator<Item = GraphChunkId>,
        workspace_id: GraphWorkspaceId,
        frontier: FrontierCheckpoint,
    ) -> Self {
        let mut chunks = ChunkSet::new();
        for id in chunk_ids {
            chunks.push(id);
        }
        Self { snapshot_id, graph_id, revision, revision_id, chunks, workspace_id, frontier }
    }

    /// Chunk identity 指纹（resume 前须与当前 snapshot 对齐）。
    pub fn chunk_identity_fingerprint(&self) -> u64 {
        self.chunks.identity_fingerprint()
    }
}
