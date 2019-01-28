//! `GcHeap`：segmented non-moving bump 堆 + object arena + tracing collect。
#![allow(unsafe_code)]

use core::cell::Cell;
use core::ptr::NonNull;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::budget::HeapBudget;
use crate::error::{GcError, Result};
use crate::header::{AllocationHeader, BlockKind, MarkState};
use crate::ids::{GcObjectId, HeapId, SegmentId};
use crate::mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcSuspendGuard};
use crate::object::{ObjectBlock, ObjectSlot, resolve_slot};
use crate::registry;
use crate::root::RootRegistry;
use crate::scratch::ScratchArena;
use crate::segment::{SegmentKind, SegmentMeta};
use crate::stats::HeapStats;
use crate::trace::{ObjectGraph, Tracer};

/// 默认 segment 容量。
const DEFAULT_SEGMENT_BYTES: usize = 256 * 1024;

struct SegmentStorage {
    meta: SegmentMeta,
    bytes: Vec<u8>,
}

/// CAS runtime heap。
pub struct GcHeap {
    id: HeapId,
    budget: HeapBudget,
    segments: Vec<Option<SegmentStorage>>,
    free_segment_slots: Vec<usize>,
    next_generation: u32,
    access_clock: u64,
    resident_bytes: usize,
    controller: Rc<GcController>,
    roots: RootRegistry,
    scratch: ScratchArena,
    stats: HeapStats,
    objects: Vec<Option<ObjectSlot>>,
    free_objects: Vec<u32>,
    /// 仅 `shared` 构造时由 Drop 注销。
    registered: bool,
}

impl GcHeap {
    /// 构造未登记的堆（须再包进 `Rc` 并 [`Self::into_shared`]）。
    fn new_inner(budget: HeapBudget) -> Self {
        Self {
            id: HeapId(0),
            budget,
            segments: Vec::new(),
            free_segment_slots: Vec::new(),
            next_generation: 1,
            access_clock: 0,
            resident_bytes: 0,
            controller: Rc::new(GcController::new()),
            roots: RootRegistry::new(),
            scratch: ScratchArena::new(),
            stats: HeapStats::default(),
            objects: Vec::new(),
            free_objects: Vec::new(),
            registered: false,
        }
    }

    /// 共享堆（登记 `HeapId`，供 numeric Drop 回找）。
    pub fn new_shared(budget: HeapBudget) -> Rc<RefCell<Self>> {
        let rc = Rc::new(RefCell::new(Self::new_inner(budget)));
        let id = registry::register(&rc);
        rc.borrow_mut().id = id;
        rc.borrow_mut().registered = true;
        rc
    }

    /// 兼容旧测试名：等价于 [`Self::new_shared`]。
    pub fn new(budget: HeapBudget) -> Rc<RefCell<Self>> {
        Self::new_shared(budget)
    }

    /// 线程默认共享堆（无显式 Session 时的 numeric 回退）。
    ///
    /// TLS 析构后不可再取；调用方应在正常执行路径使用，勿在全局 Drop 里依赖。
    pub fn shared_default() -> Rc<RefCell<Self>> {
        thread_local! {
            static DEFAULT: RefCell<Option<Rc<RefCell<GcHeap>>>> = const { RefCell::new(None) };
        }
        DEFAULT
            .try_with(|slot| {
                let mut guard = slot.borrow_mut();
                if guard.is_none() {
                    *guard = Some(Self::new_shared(HeapBudget::default()));
                }
                guard.as_ref().expect("default heap").clone()
            })
            .expect("shared_default while TLS alive")
    }

    /// 本堆 id。
    pub fn id(&self) -> HeapId {
        self.id
    }

    /// 预算。
    pub fn budget(&self) -> &HeapBudget {
        &self.budget
    }

    /// GC 控制器。
    pub fn gc(&self) -> &Rc<GcController> {
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

    /// 当前有效 mode。
    pub fn effective_mode(&self) -> GcMode {
        self.controller.effective_mode()
    }

    /// Suspend。
    pub fn suspend(&self) -> GcSuspendGuard {
        self.controller.suspend()
    }

    /// Defer。
    pub fn defer(&self) -> GcDeferGuard {
        self.controller.defer()
    }

    /// Pin segments。
    pub fn pin(&self, segments: &[SegmentId]) -> GcPinGuard {
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

    /// 分配 numeric limb block。
    pub fn allocate_numeric_block(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) =
            self.allocate_payload(SegmentKind::Numeric, BlockKind::Numeric, payload_bytes, u32::MAX)?;
        Ok(NumericBlock {
            ptr: limbs.cast(),
            capacity: capacity_limbs,
            segment_id: seg_id,
            heap_id: self.id,
        })
    }

    /// 分配 object 槽 + payload，返回 [`GcObjectId`]。
    pub fn allocate_object(&mut self, payload_bytes: usize) -> Result<GcObjectId> {
        if payload_bytes == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let index = if let Some(i) = self.free_objects.pop() {
            i
        } else {
            let i = u32::try_from(self.objects.len()).map_err(|_| GcError::InvalidCapacity)?;
            self.objects.push(None);
            i
        };
        let generation = self
            .objects
            .get(index as usize)
            .and_then(|s| s.as_ref().map(|o| o.generation.wrapping_add(1).max(1)))
            .unwrap_or(1);
        let (seg_id, ptr) =
            self.allocate_payload(SegmentKind::LongLivedObject, BlockKind::Object, payload_bytes, index)?;
        self.objects[index as usize] = Some(ObjectSlot {
            generation,
            mark: Cell::new(MarkState::White),
            block: Some(ObjectBlock {
                ptr,
                byte_len: payload_bytes,
                segment_id: seg_id,
            }),
        });
        Ok(GcObjectId { index, generation })
    }

    /// 解析 object payload。
    pub fn object_payload_mut(&mut self, id: GcObjectId) -> Result<&mut [u8]> {
        let block = {
            let slot = resolve_slot(&self.objects, id)?;
            slot.block.ok_or(GcError::StaleObject {
                index: id.index,
                expected_generation: id.generation,
            })?
        };
        // SAFETY: block 由本 heap 分配且 slot 仍存活。
        Ok(unsafe { core::slice::from_raw_parts_mut(block.ptr.as_ptr(), block.byte_len) })
    }

    /// 只读 payload。
    pub fn object_payload(&self, id: GcObjectId) -> Result<&[u8]> {
        let block = {
            let slot = resolve_slot(&self.objects, id)?;
            slot.block.ok_or(GcError::StaleObject {
                index: id.index,
                expected_generation: id.generation,
            })?
        };
        Ok(unsafe { core::slice::from_raw_parts(block.ptr.as_ptr(), block.byte_len) })
    }

    /// 显式释放 object（推进 generation，可供 stale 检测）。
    pub fn release_object(&mut self, id: GcObjectId) -> Result<()> {
        let index = id.index as usize;
        let Some(Some(slot)) = self.objects.get(index) else {
            return Err(GcError::StaleObject {
                index: id.index,
                expected_generation: id.generation,
            });
        };
        if slot.generation != id.generation || slot.block.is_none() {
            return Err(GcError::StaleObject {
                index: id.index,
                expected_generation: id.generation,
            });
        }
        if let Some(mut slot) = self.objects[index].take() {
            if let Some(block) = slot.block.take() {
                let _ = self.release_payload(block.ptr, BlockKind::Object);
            }
            let generation = slot.generation.wrapping_add(1).max(1);
            self.objects[index] = Some(ObjectSlot::vacant(generation));
            self.free_objects.push(id.index);
        }
        Ok(())
    }

    /// Limb 可写视图。
    pub fn numeric_limbs_mut(&mut self, block: &NumericBlock) -> Result<&mut [u64]> {
        self.segment_ref(block.segment_id).ok_or(GcError::UnknownAllocation)?;
        Ok(unsafe { core::slice::from_raw_parts_mut(block.ptr.as_ptr(), block.capacity) })
    }

    /// Limb 只读视图。
    pub fn numeric_limbs(&self, block: &NumericBlock) -> Result<&[u64]> {
        self.segment_ref(block.segment_id).ok_or(GcError::UnknownAllocation)?;
        Ok(unsafe { core::slice::from_raw_parts(block.ptr.as_ptr(), block.capacity) })
    }

    /// Header（limb 或 object payload 起点）。
    pub fn header_for_payload(&self, payload: NonNull<u8>) -> Result<&AllocationHeader> {
        let header = unsafe {
            payload
                .as_ptr()
                .sub(AllocationHeader::size())
                .cast::<AllocationHeader>()
        };
        let hdr = unsafe { &*header };
        if hdr.heap_id != self.id {
            return Err(GcError::UnknownAllocation);
        }
        self.segment_ref(hdr.segment_id).ok_or(GcError::UnknownAllocation)?;
        Ok(hdr)
    }

    /// 兼容旧名。
    pub fn header_for_limbs(&self, limbs: NonNull<u64>) -> Result<&AllocationHeader> {
        self.header_for_payload(limbs.cast())
    }

    /// 标记 allocation 可达。
    pub fn mark_payload(&mut self, payload: *const u8) -> Result<()> {
        if payload.is_null() {
            return Err(GcError::UnknownAllocation);
        }
        let header = unsafe {
            payload
                .sub(AllocationHeader::size())
                .cast::<AllocationHeader>() as *mut AllocationHeader
        };
        unsafe {
            (*header).mark_state = MarkState::Black;
            if (*header).block_kind == BlockKind::Object {
                let idx = (*header).object_index as usize;
                if let Some(Some(slot)) = self.objects.get(idx) {
                    slot.mark.set(MarkState::Black);
                }
            }
        }
        Ok(())
    }

    /// 标记 limbs。
    pub fn mark_limbs(&mut self, limbs: NonNull<u64>) -> Result<()> {
        self.mark_payload(limbs.as_ptr().cast())
    }

    /// 显式释放 numeric block（Rust Drop 路径）。
    pub fn release_numeric_block(&mut self, block: NumericBlock) -> Result<()> {
        self.release_payload(block.ptr.cast(), BlockKind::Numeric)
    }

    /// 经 registry 释放（`OwnedLimbBuffer::Drop`）。
    pub fn release_numeric_limbs_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| {
            let _ = heap.release_payload(limbs.cast(), BlockKind::Numeric);
        })
    }

    /// 无 tracing 的 collect（只回收空 segment）。
    pub fn collect(&mut self) -> Result<CollectReport> {
        self.collect_traced(&crate::trace::EmptyObjectGraph)
    }

    /// Tracing collect：roots → ObjectGraph → mark → sweep 未标记 object → reclaim 空 segment。
    pub fn collect_traced(&mut self, graph: &dyn ObjectGraph) -> Result<CollectReport> {
        let started = Instant::now();
        let mode = self.controller.effective_mode();
        let mut reclaimed = 0u64;
        let mut objects_swept = 0u64;

        if !matches!(mode, GcMode::Disabled) {
            self.clear_marks();
            {
                let roots: Vec<_> = self.roots.iter().collect();
                let mut tracer = MarkingTracer { heap: self, gray: Vec::new() };
                for root in roots {
                    tracer.mark_object(root.object);
                }
                while let Some(id) = tracer.gray.pop() {
                    graph.trace_object(id, &mut tracer);
                }
            }
            objects_swept = self.sweep_unmarked_objects();
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
            objects_swept,
            resident_bytes: self.resident_bytes,
            gc_time_ns: elapsed,
        })
    }

    /// 存活 segment。
    pub fn segments(&self) -> impl Iterator<Item = &SegmentMeta> {
        self.segments.iter().filter_map(|s| s.as_ref().map(|x| &x.meta))
    }

    /// Resident 字节。
    pub fn resident_bytes(&self) -> usize {
        self.resident_bytes
    }

    fn allocate_payload(
        &mut self,
        kind: SegmentKind,
        block_kind: BlockKind,
        payload_bytes: usize,
        object_index: u32,
    ) -> Result<(SegmentId, NonNull<u8>)> {
        let total = AllocationHeader::size()
            .checked_add(payload_bytes)
            .ok_or(GcError::InvalidCapacity)?;
        let (seg_index, offset) = self.bump_allocate(kind, total)?;
        let seg = self.segments[seg_index].as_mut().expect("segment");
        let seg_id = seg.meta.id;
        let header_ptr = unsafe { seg.bytes.as_mut_ptr().add(offset).cast::<AllocationHeader>() };
        unsafe {
            header_ptr.write(AllocationHeader {
                segment_id: seg_id,
                heap_id: self.id,
                block_kind,
                byte_len: u32::try_from(payload_bytes).map_err(|_| GcError::InvalidCapacity)?,
                alignment: 8,
                mark_state: MarkState::White,
                pin_state: 0,
                object_index,
            });
        }
        let payload = unsafe {
            NonNull::new_unchecked(seg.bytes.as_mut_ptr().add(offset + AllocationHeader::size()))
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
        Ok((seg_id, payload))
    }

    fn release_payload(&mut self, payload: NonNull<u8>, expected: BlockKind) -> Result<()> {
        let header = unsafe {
            payload
                .as_ptr()
                .sub(AllocationHeader::size())
                .cast::<AllocationHeader>()
        };
        let (seg_id, kind) = unsafe { ((*header).segment_id, (*header).block_kind) };
        if kind != expected {
            return Err(GcError::UnknownAllocation);
        }
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

    fn clear_marks(&mut self) {
        for slot in self.objects.iter().flatten() {
            slot.mark.set(MarkState::White);
        }
        for seg in self.segments.iter_mut().flatten() {
            let mut offset = 0usize;
            while offset + AllocationHeader::size() <= seg.meta.used {
                let header = unsafe {
                    seg.bytes
                        .as_mut_ptr()
                        .add(offset)
                        .cast::<AllocationHeader>()
                };
                unsafe {
                    (*header).mark_state = MarkState::White;
                    let step = AllocationHeader::size() + (*header).byte_len as usize;
                    offset = align_up(offset + step, 8);
                }
            }
        }
    }

    fn sweep_unmarked_objects(&mut self) -> u64 {
        let mut swept = 0u64;
        let indices: Vec<usize> = (0..self.objects.len()).collect();
        for index in indices {
            let should_sweep = self.objects[index]
                .as_ref()
                .is_some_and(|s| s.block.is_some() && s.mark.get() == MarkState::White);
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
        let capacity = bytes.max(DEFAULT_SEGMENT_BYTES).next_power_of_two();
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
        } else {
            let i = self.segments.len();
            self.segments.push(None);
            i
        };
        let id = SegmentId {
            index: index as u32,
            generation,
        };
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
            bytes: vec![0u8; capacity],
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
        (seg.meta.id == id).then_some(index)
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

    fn mark_object_id(&mut self, id: GcObjectId, gray: &mut Vec<GcObjectId>) {
        let Some(Some(slot)) = self.objects.get(id.index as usize) else {
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
            let _ = self.mark_payload(block.ptr.as_ptr());
        }
        gray.push(id);
    }
}

impl Drop for GcHeap {
    fn drop(&mut self) {
        if self.registered {
            registry::unregister(self.id);
        }
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
        let _ = self.heap.mark_payload(payload);
    }
}

/// Numeric limb block。
#[derive(Debug, Clone, Copy)]
pub struct NumericBlock {
    /// Limb 起点。
    pub ptr: NonNull<u64>,
    /// Limb 容量。
    pub capacity: usize,
    /// Segment。
    pub segment_id: SegmentId,
    /// Owner heap。
    pub heap_id: HeapId,
}

/// Collect 报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectReport {
    /// 有效 mode。
    pub mode: GcMode,
    /// 回收段数。
    pub segments_reclaimed: u64,
    /// Sweep 掉的 object 数。
    pub objects_swept: u64,
    /// Resident。
    pub resident_bytes: usize,
    /// 耗时 ns。
    pub gc_time_ns: u64,
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

/// 由 limb 指针读取 header 中的 [`HeapId`]（不校验 heap 仍存活）。
pub fn heap_id_for_limbs(limbs: NonNull<u64>) -> HeapId {
    // SAFETY: 调用方保证 ptr 指向本运行时分配的 limb 区。
    unsafe {
        let header = limbs
            .as_ptr()
            .cast::<u8>()
            .sub(AllocationHeader::size())
            .cast::<AllocationHeader>();
        (*header).heap_id
    }
}

unsafe impl Send for NumericBlock {}
