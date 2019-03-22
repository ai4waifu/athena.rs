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

/// Numeric block 生命周期（与 [`BlockKind::Numeric`] 一起解释。Object 为 [`Self::Unspecified`]）。
///
/// `RustOwned` 与 `GcOwned` 互斥：同一 pointer 不得既由 Rust `Drop` 释放又被 tracing sweep。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum NumericOwnership {
    /// 非 numeric，或尚未标记。
    #[default]
    Unspecified = 0,
    /// 仅 Rust `Drop` / `release_numeric_block` 释放。sweep 必须跳过。
    RustOwned = 1,
    /// Session / 长期值：仅 [`crate::NumericRoot`] / Trace 保活。
    /// `Drop` / `release_numeric_block` 不得 free；无 root 且未标记时由 sweep 回收。
    GcOwned = 2,
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
    /// Numeric 生命周期。Object 为 [`NumericOwnership::Unspecified`]。
    pub numeric_ownership: NumericOwnership,
}

impl AllocationHeader {
    /// Header 字节大小。
    pub const fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_aligned_8() {
        assert_eq!(AllocationHeader::size() % 8, 0);
        assert!(AllocationHeader::size() >= 24);
    }
}
