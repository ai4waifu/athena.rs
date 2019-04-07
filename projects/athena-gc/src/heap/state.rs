//! `GcHeap` 核心状态字段（无子系统实现）。

use core::cell::Cell;
use std::{collections::HashSet, rc::Rc};

use crate::{
    batch::AllocationAccounting, budget::HeapBudget, ids::HeapId, mode::GcController, object::ObjectSlot, root::RootRegistry,
    scratch::ScratchArena, stats::HeapStats,
};

use super::segment_store::SegmentStorage;

/// CAS runtime heap。
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
    /// `Drop` 遇 `HeapBusy` 的泄漏计数（与 registry 共享，可不借 `RefCell` 递增）。
    pub(super) drop_busy_leaks: Rc<Cell<u64>>,
    pub(super) objects: Vec<Option<ObjectSlot>>,
    pub(super) free_objects: Vec<u32>,
    /// GC-owned numeric payloads（不经 Rust `Drop` 释放；由 tracing sweep）。
    pub(super) traced_numeric: HashSet<usize>,
    /// 微基准：Drop 不回收，由 [`crate::GcHeap::clear_numeric_to`] rewind bump。
    pub(super) bump_ephemeral: bool,
    /// 分配记账策略（[`AllocationAccounting::Batched`] 仅由 batch 入口设置）。
    pub(super) accounting: AllocationAccounting,
    /// Batched 模式下累计字节（header+payload）。
    pub(super) batch_bytes: usize,
    /// Batched 模式下累计分配次数。
    pub(super) batch_allocs: u64,
    /// 仅 `shared` 构造时由 Drop 注销。
    pub(super) registered: bool,
}
