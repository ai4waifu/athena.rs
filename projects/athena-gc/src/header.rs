//! Allocation header 与 block 种类（位于 payload 之前）。

use crate::ids::SegmentId;

/// Block 用途（写入 header，供 tracing / reclaim）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockKind {
    /// 数值 limb 区。
    Numeric = 1,
    /// 通用对象槽（占位）。
    Object = 2,
    /// 缓存持有区（占位）。
    Cache = 3,
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
    /// Block 种类。
    pub block_kind: BlockKind,
    /// Payload 字节长度（不含本 header）。
    pub byte_len: u32,
    /// 对齐要求（记录用）。
    pub alignment: u16,
    /// Mark。
    pub mark_state: MarkState,
    /// Pin 引用计数（>0 时 segment 不可 reclaim）。
    pub pin_state: u16,
    /// 保留。
    pub _pad: u16,
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
        assert!(AllocationHeader::size() >= 16);
    }
}
