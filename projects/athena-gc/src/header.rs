//! Allocation header 与 block 种类（位于 payload 之前）。

use crate::ids::{HeapId, SegmentId};

/// Block 用途（写入 header，供 tracing / reclaim）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockKind {
    /// 数值 limb 区。
    Numeric = 1,
    /// 通用对象槽。
    Object = 2,
    /// 缓存持有区（占位）。
    Cache = 3,
    /// 图索引 payload（offsets / adjacency 等）。
    GraphIndex = 4,
    /// 图属性 / 权重列 payload。
    GraphProperty = 5,
}

/// 单一 reclaim 权限（正交属性中的 reclaim authority，不是 ownership 实体）。
///
/// 构造时写入，禁止同一 pointer 原地翻转。`ExplicitRelease` 与 `TracingSweep` 互斥。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ReclaimAuthority {
    /// 非 numeric，或尚未标记。
    #[default]
    Unspecified = 0,
    /// 仅显式 `release_numeric_block` / Rust `Drop` 释放。sweep 必须跳过。
    ExplicitRelease = 1,
    /// 已发布：仅 root / Trace 保活；无 root 且未标记时由 tracing sweep 回收。
    TracingSweep = 2,
}

/// Mark 位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum MarkState {
    /// 未标记。
    #[default]
    White = 0,
    /// 可达。
    Black = 1,
}

/// 统一 allocation 前缀（不进入 `Magnitude` union）。
///
/// 只记录物理事实：block kind、segment、大小、mark、pin、reclaim authority。
/// 不表达“谁拥有对象”。
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct AllocationHeader {
    /// 所属 segment。
    pub segment_id: SegmentId,
    /// 拥有该 segment 的 heap。
    pub heap_id: HeapId,
    /// Block 种类。
    pub block_kind: BlockKind,
    /// Payload 字节长度（不含本 header）。
    pub byte_len: u32,
    /// 对齐要求（记录用）。
    pub alignment: u16,
    /// Mark。
    pub mark_state: MarkState,
    /// Pin 引用计数（>0 时不可 sweep 该 block）。
    pub pin_state: u16,
    /// 关联 `GcObjectId.index`（Object block；Numeric 为 `u32::MAX`）。
    pub object_index: u32,
    /// Reclaim authority。Object 等非 numeric 为 [`ReclaimAuthority::Unspecified`]。
    pub reclaim_authority: ReclaimAuthority,
}

impl AllocationHeader {
    /// Header 字节大小。
    pub const fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}
