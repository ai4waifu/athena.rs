//! `GcHeap` 核心状态字段（无子系统实现）。

use core::cell::Cell;
use std::{collections::HashSet, rc::Rc};

use crate::{
    batch::AllocationAccounting, budget::HeapBudget, ids::HeapId, mode::GcController, object::ObjectSlot, root::RootRegistry,
    scratch::ScratchArena, stats::HeapStats,
};

use super::segment_store::SegmentStorage;

/// CAS 运行时堆。
pub struct GcHeap {
    pub(super) id: HeapId,
    pub(super) budget: HeapBudget,
    pub(super) segments: Vec<Option<SegmentStorage>>,
    pub(super) free_segment_slots: Vec<usize>,
    pub(super) next_generation: u32,
    pub(super) access_clock: u64,
    pub(super) resident_bytes: usize,
    pub(super) controller: Rc<GcController>,
    pub(super) roots: RootRegistry,
    pub(super) scratch: ScratchArena,
    pub(super) stats: HeapStats,
    /// `Drop` 遇 `HeapBusy` 的泄漏计数（与注册表共享，可不借 `RefCell` 递增）。
    pub(super) drop_busy_leaks: Rc<Cell<u64>>,
    pub(super) objects: Vec<Option<ObjectSlot>>,
    pub(super) free_objects: Vec<u32>,
    /// GC 持有的数值载荷（不经 Rust `Drop` 释放；由追踪清扫回收）。
    pub(super) traced_numeric: HashSet<usize>,
    /// 微基准：`Drop` 不回收，由 [`crate::GcHeap::clear_numeric_to`] 回退 bump。
    pub(super) bump_ephemeral: bool,
    /// 分配记账策略（[`AllocationAccounting::Batched`] 仅由批入口设置）。
    pub(super) accounting: AllocationAccounting,
    /// 批量模式下累计字节（头 + 载荷）。
    pub(super) batch_bytes: usize,
    /// 批量模式下累计分配次数。
    pub(super) batch_allocs: u64,
    /// 仅共享构造时由 `Drop` 注销。
    pub(super) registered: bool,
}
