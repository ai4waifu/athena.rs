//! 追踪回收：标记 / 清扫状态机（须保持连续可读）。

use core::ptr::NonNull;
use std::time::Instant;

use crate::{
    error::Result,
    header::{AllocationHeader, BlockKind, MarkState, ReclaimAuthority},
    ids::{GcObjectId, SegmentId},
    mode::GcMode,
    object::ObjectSlot,
    trace::{ObjectGraph, Tracer},
};

use super::{allocation::align_up, state::GcHeap};

/// 一次 `collect` / `collect_traced` 的汇总报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectReport {
    /// 本次回收时的有效 [`GcMode`]。
    pub mode: GcMode,
    /// 回收的空 segment 段数。
    pub segments_reclaimed: u64,
    /// 清扫掉的对象数。
    pub objects_swept: u64,
    /// 清扫掉的 GC 持有 numeric 块数。
    pub numeric_blocks_swept: u64,
    /// 回收后的驻留字节数。
    pub resident_bytes: usize,
    /// 耗时（纳秒）。
    pub gc_time_ns: u64,
    /// 峰值 arena 字节（报告用）。
    pub peak_arena_bytes: usize,
    /// 峰值 scratch 字节（报告用）。
    pub peak_scratch_bytes: usize,
}

impl GcHeap {
    /// 无对象图追踪的回收（仅回收空 segment）。
    pub fn collect(&mut self) -> Result<CollectReport> {
        self.collect_traced(&crate::trace::EmptyObjectGraph)
    }

    /// 追踪回收：根 → 对象图 → 标记 → 清扫对象/数值块 → 回收空 segment。
    pub fn collect_traced(&mut self, graph: &dyn ObjectGraph) -> Result<CollectReport> {
        let started = Instant::now();
        let mode = self.controller.effective_mode();
        let mut reclaimed = 0u64;
        let mut objects_swept = 0u64;
        let mut numeric_swept = 0u64;

        if !matches!(mode, GcMode::Disabled) {
            self.clear_marks();
            {
                let roots: Vec<_> = self.roots.iter().collect();
                let numeric_roots: Vec<_> = self.roots.iter_numeric().collect();
                let mut tracer = MarkingTracer { heap: self, gray: Vec::new() };
                for root in roots {
                    tracer.mark_object(root.object);
                }
                for root in numeric_roots {
                    tracer.mark_allocation(root.payload.as_ptr());
                }
                while let Some(id) = tracer.gray.pop() {
                    graph.trace_object(id, &mut tracer);
                }
            }
            objects_swept = self.sweep_unmarked_objects();
            numeric_swept = self.sweep_unmarked_traced_numeric();
            let ids: Vec<SegmentId> = self.segments.iter().filter_map(|s| s.as_ref().map(|x| x.meta.id)).collect();
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
            objects_swept,
            numeric_blocks_swept: numeric_swept,
            resident_bytes: self.resident_bytes,
            gc_time_ns: elapsed,
            peak_arena_bytes: self.stats.peak_arena_bytes,
            peak_scratch_bytes: self.stats.peak_scratch_bytes,
        })
    }

    pub(super) fn clear_marks(&mut self) {
        for slot in self.objects.iter().flatten() {
            slot.mark.set(MarkState::White);
        }
        for seg in self.segments.iter_mut().flatten() {
            let mut offset = 0usize;
            while offset + AllocationHeader::size() <= seg.meta.used {
                let header = unsafe { seg.bytes.as_mut_ptr().add(offset).cast::<AllocationHeader>() };
                unsafe {
                    (*header).mark_state = MarkState::White;
                    let step = AllocationHeader::size() + (*header).byte_len as usize;
                    offset = align_up(offset + step, 8);
                }
            }
        }
    }

    pub(super) fn sweep_unmarked_objects(&mut self) -> u64 {
        let mut swept = 0u64;
        let indices: Vec<usize> = (0..self.objects.len()).collect();
        for index in indices {
            let should_sweep = self.objects[index].as_ref().is_some_and(|s| s.block.is_some() && s.mark.get() == MarkState::White);
            if !should_sweep {
                continue;
            }
            if let Some(mut slot) = self.objects[index].take() {
                if let Some(block) = slot.block.take() {
                    let _ = self.release_payload(block.ptr, BlockKind::Object);
                }
                let generation = slot.generation.wrapping_add(1).max(1);
                self.objects[index] = Some(ObjectSlot::vacant(generation));
                self.free_objects.push(index as u32);
                swept = swept.saturating_add(1);
            }
        }
        swept
    }

    /// 回收未标记的 GC 持有数值块（Rust 持有者不在 `traced_numeric` 内）。
    pub(super) fn sweep_unmarked_traced_numeric(&mut self) -> u64 {
        let candidates: Vec<usize> = self.traced_numeric.iter().copied().collect();
        let mut swept = 0u64;
        for addr in candidates {
            let Some(ptr) = NonNull::new(addr as *mut u8)
            else {
                continue;
            };
            let header = unsafe { ptr.as_ptr().sub(AllocationHeader::size()).cast::<AllocationHeader>() };
            let (kind, mark, pin) = unsafe { ((*header).block_kind, (*header).mark_state, (*header).pin_state) };
            if kind != BlockKind::Numeric || mark != MarkState::White || pin > 0 {
                continue;
            }
            let authority = unsafe { (*header).reclaim_authority };
            if authority != ReclaimAuthority::TracingSweep {
                continue;
            }
            if !self.traced_numeric.remove(&addr) {
                continue;
            }
            if self.release_payload(ptr.cast(), BlockKind::Numeric).is_ok() {
                swept = swept.saturating_add(1);
            }
        }
        swept
    }

    pub(super) fn mark_object_id(&mut self, id: GcObjectId, gray: &mut Vec<GcObjectId>) {
        let Some(Some(slot)) = self.objects.get(id.index as usize)
        else {
            return;
        };
        if slot.generation != id.generation || slot.block.is_none() {
            return;
        }
        if slot.mark.get() == MarkState::Black {
            return;
        }
        slot.mark.set(MarkState::Black);
        if let Some(block) = slot.block {
            let _ = self.mark_payload(block.ptr);
        }
        gray.push(id);
    }
}

struct MarkingTracer<'a> {
    heap: &'a mut GcHeap,
    gray: Vec<GcObjectId>,
}

impl Tracer for MarkingTracer<'_> {
    fn mark_object(&mut self, id: GcObjectId) {
        self.heap.mark_object_id(id, &mut self.gray);
    }

    fn mark_allocation(&mut self, payload: *const u8) {
        if let Some(ptr) = NonNull::new(payload.cast_mut()) {
            let _ = self.heap.mark_payload(ptr);
        }
    }
}
