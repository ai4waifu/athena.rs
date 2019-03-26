//! Engine 侧图驻留策略：spill / LRU / checkpoint 接线。
//!
//! 形状与 Trace 在 `athena-graph`；本模块决定何时 spill、何时 materialize、如何绑定 checkpoint。

use std::collections::VecDeque;

use athena_gc::GcHeap;
use athena_graph::{
    allocate_spill_id, allocate_workspace_id, ChunkRegistry, ChunkResidency, FrontierCheckpoint, GraphAlgorithmCheckpoint,
    GraphError, GraphPublication, GraphWorkspaceId, SpillObjectId,
};

/// 图 chunk 驻留 / spill 策略控制器。
#[derive(Debug, Clone)]
pub struct GraphResidencyController {
    /// 同时允许的 resident chunk 上限（超过则 LRU spill）。
    pub max_resident_chunks: usize,
    lru: VecDeque<athena_graph::GraphChunkId>,
}

impl Default for GraphResidencyController {
    fn default() -> Self {
        Self::new(64)
    }
}

impl GraphResidencyController {
    /// 构造；`max_resident_chunks == 0` 视为 1。
    pub fn new(max_resident_chunks: usize) -> Self {
        Self {
            max_resident_chunks: max_resident_chunks.max(1),
            lru: VecDeque::new(),
        }
    }

    /// 记录一次访问（lease / 算法读路径调用）。
    pub fn touch(&mut self, chunk: athena_graph::GraphChunkId) {
        self.lru.retain(|c| *c != chunk);
        self.lru.push_back(chunk);
    }

    /// 当前 LRU 队列长度（诊断）。
    pub fn tracked_len(&self) -> usize {
        self.lru.len()
    }

    /// 若 resident 超限则对最久未用且无 pin 的 chunk 执行 spill。
    ///
    /// 真实 I/O 由调用方后续替换；此处分配 [`SpillObjectId`] 并更新 registry 状态机。
    pub fn enforce_resident_limit(
        &mut self,
        heap: &mut GcHeap,
        registry: &mut ChunkRegistry,
    ) -> Result<Vec<(athena_graph::GraphChunkId, SpillObjectId)>, GraphError> {
        let mut spilled = Vec::new();
        loop {
            let resident = self.count_resident(registry);
            if resident <= self.max_resident_chunks {
                break;
            }
            let Some(victim) = self.pop_evictable(registry) else {
                break;
            };
            if registry.get(victim).map(|m| m.pin_count > 0).unwrap_or(true) {
                // 仍 pin：推回队尾，停止本轮以免忙等。
                self.lru.push_back(victim);
                break;
            }
            let _ = registry.mark_evictable(victim);
            let spill_id = allocate_spill_id(heap)?;
            registry.spill(victim, spill_id)?;
            spilled.push((victim, spill_id));
        }
        Ok(spilled)
    }

    /// 需要读时 materialize（若已 Spilled）。
    pub fn ensure_resident(
        &mut self,
        registry: &mut ChunkRegistry,
        chunk: athena_graph::GraphChunkId,
    ) -> Result<(), GraphError> {
        let residency = registry
            .get(chunk)
            .map(|m| m.residency)
            .ok_or(GraphError::UnknownChunk { chunk })?;
        if residency == ChunkResidency::Spilled || residency == ChunkResidency::Evictable {
            registry.materialize(chunk)?;
        }
        self.touch(chunk);
        Ok(())
    }

    fn count_resident(&self, registry: &ChunkRegistry) -> usize {
        self.lru
            .iter()
            .filter(|id| {
                registry
                    .get(**id)
                    .map(|m| matches!(m.residency, ChunkResidency::Resident | ChunkResidency::Mapped))
                    .unwrap_or(false)
            })
            .count()
    }

    fn pop_evictable(&mut self, registry: &ChunkRegistry) -> Option<athena_graph::GraphChunkId> {
        let mut skipped = VecDeque::new();
        let victim = loop {
            let Some(id) = self.lru.pop_front() else {
                break None;
            };
            let Some(meta) = registry.get(id) else {
                continue;
            };
            if matches!(meta.residency, ChunkResidency::Resident | ChunkResidency::Mapped) && meta.pin_count == 0 {
                break Some(id);
            }
            skipped.push_back(id);
        };
        while let Some(id) = skipped.pop_front() {
            self.lru.push_front(id);
        }
        victim
    }
}

/// 由已发布图与 frontier 态构造可恢复 checkpoint（不含裸指针）。
pub fn bind_algorithm_checkpoint(
    publication: &GraphPublication,
    heap: &mut GcHeap,
    frontier: FrontierCheckpoint,
) -> Result<(GraphAlgorithmCheckpoint, GraphWorkspaceId), GraphError> {
    let workspace_id = allocate_workspace_id(heap)?;
    let checkpoint = GraphAlgorithmCheckpoint {
        snapshot_id: publication.snapshot_id,
        graph_id: publication.graph_id,
        revision: publication.revision,
        revision_id: publication.revision_id,
        chunks: publication.chunks.clone(),
        workspace_id,
        frontier,
    };
    Ok((checkpoint, workspace_id))
}

/// Resume：校验 snapshot / revision / chunk identity 后恢复 frontier（须重新获取 lease，禁止沿用旧 slice）。
pub fn resume_from_algorithm_checkpoint<N, E>(
    graph: &athena_graph::MutableGraph<N, E>,
    publication: &GraphPublication,
    checkpoint: GraphAlgorithmCheckpoint,
    cancel: Option<&athena_graph::CancelFlag>,
) -> Result<athena_graph::DeterministicBfsOutcome, GraphError> {
    if checkpoint.snapshot_id != publication.snapshot_id
        || checkpoint.revision_id != publication.revision_id
        || checkpoint.revision != publication.revision
        || checkpoint.chunk_identity_fingerprint() != publication.chunks.identity_fingerprint()
    {
        return Err(GraphError::CheckpointIdentityMismatch);
    }
    athena_graph::resume_deterministic_bfs(graph, checkpoint.frontier, cancel)
}
