//! ChunkSet 与 chunk 元数据形状。

use athena_gc::{GcObjectId, Trace, Tracer};

use super::{
    ids::{GraphChunkId, SpillObjectId},
    residency::ChunkResidency,
};

/// Chunk 元数据（可达性 · 驻留 · lease/pin · COW 共享计数）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkMeta {
    /// Chunk 身份。
    pub id: GraphChunkId,
    /// 语义可达（仍属于活的 snapshot / workspace）。
    pub semantic_reachable: bool,
    /// 驻留状态。
    pub residency: ChunkResidency,
    /// 可选 spill backing。
    pub spill: Option<SpillObjectId>,
    /// 活跃 lease 数。
    pub lease_count: u32,
    /// 活跃 resident pin 数。
    pub pin_count: u32,
    /// COW 共享计数（≥1 表示至少一处引用）。
    pub share_count: u32,
}

impl ChunkMeta {
    /// 新建 resident、可达、独占引用的 chunk。
    pub fn new_resident(id: GraphChunkId) -> Self {
        Self { id, semantic_reachable: true, residency: ChunkResidency::Resident, spill: None, lease_count: 0, pin_count: 0, share_count: 1 }
    }
}

impl Trace for ChunkMeta {
    fn trace(&self, tracer: &mut dyn Tracer) {
        tracer.mark_object(self.id.as_object());
        if let Some(spill) = self.spill {
            tracer.mark_object(spill.as_object());
        }
    }
}

/// Snapshot 持有的 chunk 集合（可共享；COW 由 [`super::ChunkRegistry`] 记账）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChunkSet {
    /// 可选 Trace 对象身份（登记进 heap 对象表时填写）。
    pub object: Option<GcObjectId>,
    /// 有序 chunk 列表（稳定序便于指纹 / checkpoint）。
    pub chunks: Vec<GraphChunkId>,
}

impl ChunkSet {
    /// 空集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 带 Trace 身份的集合。
    pub fn with_object(object: GcObjectId, chunks: Vec<GraphChunkId>) -> Self {
        Self { object: Some(object), chunks }
    }

    /// 追加 chunk id（调用方负责 registry.share）。
    pub fn push(&mut self, id: GraphChunkId) {
        self.chunks.push(id);
    }

    /// 稳定指纹（仅 identity，不含驻留态）。
    pub fn identity_fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for c in &self.chunks {
            h ^= u64::from(c.object.index);
            h = h.wrapping_mul(0x100_0000_01b3);
            h ^= u64::from(c.object.generation);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        h
    }
}

impl Trace for ChunkSet {
    fn trace(&self, tracer: &mut dyn Tracer) {
        if let Some(object) = self.object {
            tracer.mark_object(object);
        }
        for chunk in &self.chunks {
            tracer.mark_object(chunk.as_object());
        }
    }
}
