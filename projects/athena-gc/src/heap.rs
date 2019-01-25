//! `GcHeap`：segmented non-moving bump 堆（bootstrap）。
#![allow(unsafe_code)]

use core::cell::Cell;
use core::ptr::NonNull;
use std::time::Instant;

use crate::budget::HeapBudget;
use crate::error::{GcError, Result};
use crate::header::{AllocationHeader, BlockKind, MarkState};
use crate::ids::SegmentId;
use crate::mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcSuspendGuard};
use crate::root::RootRegistry;
use crate::scratch::ScratchArena;
use crate::segment::{SegmentKind, SegmentMeta};
use crate::stats::HeapStats;

/// 默认 numeric segment 容量。
const DEFAULT_NUMERIC_SEGMENT_BYTES: usize = 256 * 1024;

struct SegmentStorage {
    meta: SegmentMeta,
    /// 拥有字节（bump 区）。
    bytes: Vec<u8>,
}

/// CAS runtime heap（object / numeric / scratch 视图入口）。
pub struct GcHeap {
    budget: HeapBudget,
    segments: Vec<Option<SegmentStorage>>,
    free_segment_slots: Vec<usize>,
    next_generation: u32,
    access_clock: u64,
    /// 当前 resident（各存活 segment capacity 之和）。
    resident_bytes: usize,
    controller: GcController,
    roots: RootRegistry,
    scratch: ScratchArena,
    stats: HeapStats,
}

impl Default for GcHeap {
    fn default() -> Self {
        Self::new(HeapBudget::default())
    }
}

impl GcHeap {
    /// 使用给定预算构造。
    pub fn new(budget: HeapBudget) -> Self {
        Self {
            budget,
            segments: Vec::new(),
            free_segment_slots: Vec::new(),
            next_generation: 1,
            access_clock: 0,
            resident_bytes: 0,
            controller: GcController::new(),
            roots: RootRegistry::new(),
            scratch: ScratchArena::new(),
            stats: HeapStats::default(),
        }
    }

    /// 预算。
    pub fn budget(&self) -> &HeapBudget {
        &self.budget
    }

    /// GC 控制器。
    pub fn gc(&self) -> &GcController {
        &self.controller
    }

    /// Root 表。
    pub fn roots(&self) -> &RootRegistry {
        &self.roots
    }

    /// Root 表可变。
    pub fn roots_mut(&mut self) -> &mut RootRegistry {
        &mut self.roots
    }

    /// Scratch。
    pub fn scratch(&mut self) -> &mut ScratchArena {
        &mut self.scratch
    }

    /// 统计。
    pub fn stats(&self) -> HeapStats {
        let mut s = self.stats;
        s.peak_scratch_bytes = s.peak_scratch_bytes.max(self.scratch.peak_bytes());
        s
    }

    /// 当前有效 `GcMode`。
    pub fn effective_mode(&self) -> GcMode {
        self.controller.effective_mode()
    }

    /// Suspend（Disabled）。
    pub fn suspend(&self) -> GcSuspendGuard<'_> {
        self.controller.suspend()
    }

    /// Defer。
    pub fn defer(&self) -> GcDeferGuard<'_> {
        self.controller.defer()
    }

    /// Pin 若干 segment。
    pub fn pin(&self, segments: &[SegmentId]) -> GcPinGuard<'_> {
        GcPinGuard::new(self, segments.to_vec())
    }

    pub(crate) fn pin_segment(&self, id: SegmentId) {
        if let Some(seg) = self.segment_ref(id) {
            seg.meta.pin_count.set(seg.meta.pin_count.get().saturating_add(1));
        }
    }

    pub(crate) fn unpin_segment(&self, id: SegmentId) {
        if let Some(seg) = self.segment_ref(id) {
            seg.meta.pin_count.set(seg.meta.pin_count.get().saturating_sub(1));
        }
    }

    /// 分配 numeric limb block，返回指向 **limb 起点** 的指针与 capacity（limb 数）。
    ///
    /// Header 位于 `ptr` 之前；`len` 由调用方 metadata 保存。
    pub fn allocate_numeric_block(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(GcError::InvalidCapacity)?;
        let total = AllocationHeader::size()
            .checked_add(payload_bytes)
            .ok_or(GcError::InvalidCapacity)?;

        let (seg_index, offset) = self.bump_allocate(SegmentKind::Numeric, total)?;
        let seg = self.segments[seg_index].as_mut().expect("just allocated");
        let seg_id = seg.meta.id;
        let header_ptr = unsafe { seg.bytes.as_mut_ptr().add(offset).cast::<AllocationHeader>() };
        unsafe {
            header_ptr.write(AllocationHeader {
                segment_id: seg_id,
                block_kind: BlockKind::Numeric,
                byte_len: u32::try_from(payload_bytes).map_err(|_| GcError::InvalidCapacity)?,
                alignment: 8,
                mark_state: MarkState::White,
                pin_state: 0,
                _pad: 0,
            });
        }
        let limbs = unsafe {
            NonNull::new_unchecked(
                seg.bytes
                    .as_mut_ptr()
                    .add(offset + AllocationHeader::size())
                    .cast::<u64>(),
            )
        };
        seg.meta.live_count = seg.meta.live_count.saturating_add(1);
        self.touch(seg_index);

        self.controller.record_allocation(total);
        self.stats.allocation_count = self.stats.allocation_count.saturating_add(1);
        self.stats.total_arena_bytes_allocated =
            self.stats.total_arena_bytes_allocated.saturating_add(total);

        if self.controller.should_collect_after_alloc() {
            let _ = self.collect();
        }

        Ok(NumericBlock {
            ptr: limbs,
            capacity: capacity_limbs,
            segment_id: seg_id,
        })
    }

    /// 由 limb 指针取 header（须为本 heap 分配）。
    pub fn header_for_limbs(&self, limbs: NonNull<u64>) -> Result<&AllocationHeader> {
        let header = unsafe {
            limbs
                .as_ptr()
                .cast::<u8>()
                .sub(AllocationHeader::size())
                .cast::<AllocationHeader>()
        };
        let hdr = unsafe { &*header };
        self.segment_ref(hdr.segment_id).ok_or(GcError::UnknownAllocation)?;
        Ok(hdr)
    }

    /// 标记 allocation 可达（tracing 用）。
    pub fn mark_limbs(&mut self, limbs: NonNull<u64>) -> Result<()> {
        let header = unsafe {
            limbs
                .as_ptr()
                .cast::<u8>()
                .sub(AllocationHeader::size())
                .cast::<AllocationHeader>()
        };
        unsafe {
            (*header).mark_state = MarkState::Black;
        }
        Ok(())
    }

    /// 释放一个 numeric block 的逻辑所有权（live_count--；不移动其它对象）。
    pub fn release_numeric_block(&mut self, block: NumericBlock) -> Result<()> {
        let header = unsafe {
            block
                .ptr
                .as_ptr()
                .cast::<u8>()
                .sub(AllocationHeader::size())
                .cast::<AllocationHeader>()
        };
        let seg_id = unsafe { (*header).segment_id };
        let Some(seg) = self.segment_mut(seg_id) else {
            return Err(GcError::UnknownAllocation);
        };
        if seg.meta.live_count > 0 {
            seg.meta.live_count -= 1;
        }
        unsafe {
            (*header).mark_state = MarkState::White;
        }
        if matches!(self.controller.effective_mode(), GcMode::Auto) {
            self.try_reclaim_segment(seg_id);
        }
        Ok(())
    }

    /// 显式收集：在非 Disabled 下回收空 segment；清除 pressure。
    pub fn collect(&mut self) -> Result<CollectReport> {
        let started = Instant::now();
        let mode = self.controller.effective_mode();
        let mut reclaimed = 0u64;

        if !matches!(mode, GcMode::Disabled) {
            let ids: Vec<SegmentId> = self
                .segments
                .iter()
                .filter_map(|s| s.as_ref().map(|x| x.meta.id))
                .collect();
            for id in ids {
                if self.try_reclaim_segment(id) {
                    reclaimed = reclaimed.saturating_add(1);
                }
            }
        }

        self.controller.clear_pressure();
        let elapsed = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.stats.collect_count = self.stats.collect_count.saturating_add(1);
        self.stats.gc_time_ns = self.stats.gc_time_ns.saturating_add(elapsed);
        self.stats.segments_reclaimed = self.stats.segments_reclaimed.saturating_add(reclaimed);
        self.stats.peak_scratch_bytes = self.stats.peak_scratch_bytes.max(self.scratch.peak_bytes());

        Ok(CollectReport {
            mode,
            segments_reclaimed: reclaimed,
            resident_bytes: self.resident_bytes,
            gc_time_ns: elapsed,
        })
    }

    /// 存活 segment 元数据迭代。
    pub fn segments(&self) -> impl Iterator<Item = &SegmentMeta> {
        self.segments.iter().filter_map(|s| s.as_ref().map(|x| &x.meta))
    }

    /// 当前 resident 字节。
    pub fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    fn bump_allocate(&mut self, kind: SegmentKind, bytes: usize) -> Result<(usize, usize)> {
        for (index, slot) in self.segments.iter_mut().enumerate() {
            let Some(seg) = slot.as_mut() else { continue };
            if seg.meta.kind != kind {
                continue;
            }
            let aligned_used = align_up(seg.meta.used, 8);
            if aligned_used.saturating_add(bytes) <= seg.meta.capacity {
                seg.meta.used = aligned_used + bytes;
                return Ok((index, aligned_used));
            }
        }

        let capacity = bytes.max(DEFAULT_NUMERIC_SEGMENT_BYTES).next_power_of_two();
        let index = self.alloc_segment(kind, capacity)?;
        let seg = self.segments[index].as_mut().expect("new segment");
        seg.meta.used = bytes;
        Ok((index, 0))
    }

    fn alloc_segment(&mut self, kind: SegmentKind, capacity: usize) -> Result<usize> {
        let count = self.segments.iter().filter(|s| s.is_some()).count() + 1;
        self.budget.check_segment_count(count)?;
        self.budget.check_arena_bytes(self.resident_bytes, capacity)?;

        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let index = if let Some(free) = self.free_segment_slots.pop() {
            free
        }
        else {
            let i = self.segments.len();
            self.segments.push(None);
            i
        };

        let id = SegmentId {
            index: index as u32,
            generation,
        };
        let mut bytes = Vec::with_capacity(capacity);
        bytes.resize(capacity, 0);
        self.segments[index] = Some(SegmentStorage {
            meta: SegmentMeta {
                id,
                kind,
                capacity,
                used: 0,
                live_count: 0,
                pin_count: Cell::new(0),
                last_access: self.access_clock,
            },
            bytes,
        });
        self.resident_bytes = self.resident_bytes.saturating_add(capacity);
        self.stats.peak_arena_bytes = self.stats.peak_arena_bytes.max(self.resident_bytes);
        self.stats.segments_allocated = self.stats.segments_allocated.saturating_add(1);
        Ok(index)
    }

    fn try_reclaim_segment(&mut self, id: SegmentId) -> bool {
        let Some(index) = self.resolve_index(id) else {
            return false;
        };
        let Some(seg) = self.segments[index].as_ref() else {
            return false;
        };
        if !seg.meta.is_reclaimable() {
            return false;
        }
        let capacity = seg.meta.capacity;
        self.segments[index] = None;
        self.free_segment_slots.push(index);
        self.resident_bytes = self.resident_bytes.saturating_sub(capacity);
        true
    }

    fn resolve_index(&self, id: SegmentId) -> Option<usize> {
        let index = id.index as usize;
        let seg = self.segments.get(index)?.as_ref()?;
        if seg.meta.id == id {
            Some(index)
        }
        else {
            None
        }
    }

    fn segment_ref(&self, id: SegmentId) -> Option<&SegmentStorage> {
        let index = self.resolve_index(id)?;
        self.segments[index].as_ref()
    }

    fn segment_mut(&mut self, id: SegmentId) -> Option<&mut SegmentStorage> {
        let index = self.resolve_index(id)?;
        self.segments[index].as_mut()
    }

    fn touch(&mut self, index: usize) {
        self.access_clock = self.access_clock.wrapping_add(1);
        if let Some(seg) = self.segments[index].as_mut() {
            seg.meta.last_access = self.access_clock;
        }
    }
}

/// 已分配 numeric limb block（所有权在调用方；释放走 `release_numeric_block`）。
#[derive(Debug, Clone, Copy)]
pub struct NumericBlock {
    /// Limb 起点（header 在前方）。
    pub ptr: NonNull<u64>,
    /// Limb 容量。
    pub capacity: usize,
    /// 所属 segment（便于 pin）。
    pub segment_id: SegmentId,
}

/// `collect` 报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectReport {
    /// 收集时有效 mode。
    pub mode: GcMode,
    /// 回收段数。
    pub segments_reclaimed: u64,
    /// 回收后 resident。
    pub resident_bytes: usize,
    /// 本次耗时纳秒。
    pub gc_time_ns: u64,
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

// Session 单线程合同；block 可跨函数传递。
unsafe impl Send for NumericBlock {}
