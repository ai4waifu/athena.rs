//! Segment 元数据与种类。

use core::cell::Cell;

use crate::ids::SegmentId;

/// Segment 用途分区。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    /// 短命对象。
    ShortLivedObject,
    /// 长命对象。
    LongLivedObject,
    /// 数值 limb。
    Numeric,
    /// 缓存区。
    Cache,
}

/// 单段堆元数据（payload 存在 `SegmentStorage`）。
#[derive(Debug)]
pub struct SegmentMeta {
    /// 稳定 id。
    pub id: SegmentId,
    /// 种类。
    pub kind: SegmentKind,
    /// 容量（字节）。
    pub capacity: usize,
    /// 已 bump 使用字节。
    pub used: usize,
    /// 存活 allocation 数。
    pub live_count: u32,
    /// Segment 级 pin（kernel 持有 raw pointer 时）。
    pub pin_count: Cell<u32>,
    /// 最近访问（单调计数，由 heap 维护）。
    pub last_access: u64,
}

impl SegmentMeta {
    /// 是否可整体 reclaim（无存活、无 pin）。
    pub fn is_reclaimable(&self) -> bool {
        self.live_count == 0 && self.pin_count.get() == 0
    }

    /// 剩余可 bump 字节。
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.used)
    }
}
