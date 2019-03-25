//! `ChunkLease` / `ResidentPin` 与 [`GcRootToken`] 分工。
//!
//! Guard 为票据（不长期借住 [`ChunkRegistry`]），以便在持有 lease/pin 时仍可推进驻留状态机。
//! 调用顺序：root → [`ChunkRegistry::acquire_lease`] → materialize → [`ChunkRegistry::pin_resident`] → scan → release。

use std::collections::HashMap;

use crate::GraphError;

use super::{
    chunk::ChunkMeta,
    ids::{GraphChunkId, SpillObjectId},
    residency::ChunkResidency,
};

/// Living 合同名：语义对象不被 tracing 回收。实现即 [`athena_gc::RootToken`]。
pub type GcRootToken = athena_gc::RootToken;

/// 管理 chunk 生命周期合同（in-core 骨架；真实 spill 由 engine 接线）。
#[derive(Debug, Default)]
pub struct ChunkRegistry {
    chunks: HashMap<GraphChunkId, ChunkMeta>,
}

impl ChunkRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记新 chunk（初始 resident + 可达 + share=1）。
    pub fn register_resident(&mut self, id: GraphChunkId) -> Result<(), GraphError> {
        if self.chunks.contains_key(&id) {
            return Err(GraphError::DuplicateChunk { chunk: id });
        }
        self.chunks.insert(id, ChunkMeta::new_resident(id));
        Ok(())
    }

    /// 查询元数据。
    pub fn get(&self, id: GraphChunkId) -> Option<&ChunkMeta> {
        self.chunks.get(&id)
    }

    /// 标记语义可达 / 不可达（≠ 是否驻留）。
    pub fn set_reachable(&mut self, id: GraphChunkId, reachable: bool) -> Result<(), GraphError> {
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        meta.semantic_reachable = reachable;
        Ok(())
    }

    /// 增加 COW 共享（多 snapshot 共享同一 chunk）。
    pub fn share(&mut self, id: GraphChunkId) -> Result<(), GraphError> {
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        meta.share_count = meta.share_count.saturating_add(1);
        Ok(())
    }

    /// 释放一处 COW 引用。`share_count` 归零且不可达、无 lease/pin 时可移除元数据。
    pub fn unshare(&mut self, id: GraphChunkId) -> Result<(), GraphError> {
        let remove = {
            let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
            meta.share_count = meta.share_count.saturating_sub(1);
            meta.share_count == 0 && !meta.semantic_reachable && meta.lease_count == 0 && meta.pin_count == 0
        };
        if remove {
            self.chunks.remove(&id);
        }
        Ok(())
    }

    /// COW fork：独占则复用同一 id。共享则分配新 chunk 并降低旧 share。
    pub fn fork_cow(&mut self, id: GraphChunkId) -> Result<GraphChunkId, GraphError> {
        let share = self.chunks.get(&id).ok_or(GraphError::UnknownChunk { chunk: id })?.share_count;
        if share <= 1 {
            return Ok(id);
        }
        let old = self.chunks.get(&id).expect("checked").clone();
        let new_id = GraphChunkId::allocate();
        let mut neu = old;
        neu.id = new_id;
        neu.share_count = 1;
        neu.lease_count = 0;
        neu.pin_count = 0;
        self.chunks.insert(new_id, neu);
        self.unshare(id)?;
        Ok(new_id)
    }

    /// Resident → Spilled（无 pin；可达时保留 metadata / spill id）。
    pub fn spill(&mut self, id: GraphChunkId, spill: SpillObjectId) -> Result<(), GraphError> {
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        if !meta.semantic_reachable {
            return Err(GraphError::ChunkUnreachable { chunk: id });
        }
        if meta.pin_count > 0 {
            return Err(GraphError::ChunkPinned { chunk: id });
        }
        if !matches!(meta.residency, ChunkResidency::Resident | ChunkResidency::Evictable | ChunkResidency::Mapped) {
            return Err(GraphError::InvalidResidencyTransition {
                chunk: id,
                from: meta.residency,
                to: ChunkResidency::Spilled,
            });
        }
        meta.spill = Some(spill);
        meta.residency = ChunkResidency::Spilled;
        Ok(())
    }

    /// Spilled / Evictable → Loading → Resident（骨架：一步到位 Resident）。
    pub fn materialize(&mut self, id: GraphChunkId) -> Result<(), GraphError> {
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        if !meta.semantic_reachable {
            return Err(GraphError::ChunkUnreachable { chunk: id });
        }
        if !meta.residency.can_materialize() && meta.residency != ChunkResidency::Loading {
            return Err(GraphError::InvalidResidencyTransition {
                chunk: id,
                from: meta.residency,
                to: ChunkResidency::Resident,
            });
        }
        meta.residency = ChunkResidency::Loading;
        meta.residency = ChunkResidency::Resident;
        Ok(())
    }

    /// Resident → Evictable（无 pin）。
    pub fn mark_evictable(&mut self, id: GraphChunkId) -> Result<(), GraphError> {
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        if meta.pin_count > 0 {
            return Err(GraphError::ChunkPinned { chunk: id });
        }
        if meta.residency != ChunkResidency::Resident {
            return Err(GraphError::InvalidResidencyTransition {
                chunk: id,
                from: meta.residency,
                to: ChunkResidency::Evictable,
            });
        }
        meta.residency = ChunkResidency::Evictable;
        Ok(())
    }

    /// 获取 chunk lease：保证 metadata / backing 在访问期内有效。
    pub fn acquire_lease(&mut self, id: GraphChunkId) -> Result<ChunkLeaseGuard, GraphError> {
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        if !meta.semantic_reachable {
            return Err(GraphError::ChunkUnreachable { chunk: id });
        }
        meta.lease_count = meta.lease_count.saturating_add(1);
        Ok(ChunkLeaseGuard { chunk_id: id })
    }

    /// 释放 lease（须成对调用；丢弃票据而不释放会泄漏计数）。
    pub fn release_lease(&mut self, lease: ChunkLeaseGuard) {
        if let Some(meta) = self.chunks.get_mut(&lease.chunk_id) {
            meta.lease_count = meta.lease_count.saturating_sub(1);
        }
    }

    /// 在已持有 lease 时 pin resident 地址。
    pub fn pin_resident(&mut self, lease: &ChunkLeaseGuard) -> Result<ResidentPinGuard, GraphError> {
        let id = lease.chunk_id;
        let meta = self.chunks.get_mut(&id).ok_or(GraphError::UnknownChunk { chunk: id })?;
        if !meta.semantic_reachable {
            return Err(GraphError::ChunkUnreachable { chunk: id });
        }
        if meta.lease_count == 0 {
            return Err(GraphError::ChunkLeaseRequired { chunk: id });
        }
        if !meta.residency.has_address() {
            return Err(GraphError::ChunkNotResident { chunk: id, residency: meta.residency });
        }
        meta.pin_count = meta.pin_count.saturating_add(1);
        Ok(ResidentPinGuard { chunk_id: id })
    }

    /// 释放 resident pin。
    pub fn release_pin(&mut self, pin: ResidentPinGuard) {
        if let Some(meta) = self.chunks.get_mut(&pin.chunk_id) {
            meta.pin_count = meta.pin_count.saturating_sub(1);
        }
    }
}

/// 保证 chunk backing / metadata 在本次访问期间有效。
#[derive(Debug)]
#[must_use = "release via ChunkRegistry::release_lease"]
pub struct ChunkLeaseGuard {
    chunk_id: GraphChunkId,
}

impl ChunkLeaseGuard {
    /// 当前 chunk。
    pub const fn chunk_id(&self) -> GraphChunkId {
        self.chunk_id
    }
}

/// 保证当前 resident 内存地址不被 eviction / reload 替换。
#[derive(Debug)]
#[must_use = "release via ChunkRegistry::release_pin"]
pub struct ResidentPinGuard {
    chunk_id: GraphChunkId,
}

impl ResidentPinGuard {
    /// 当前 chunk。
    pub const fn chunk_id(&self) -> GraphChunkId {
        self.chunk_id
    }
}
