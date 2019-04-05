//! `GcHeap`：segmented non-moving bump 堆 + object arena + tracing collect。
//!
//! Living `23` 批次 2：`state` / `segment_store` / `allocation` 已拆出；其余子系统仍在本文件，后续批次迁出。
#![allow(unsafe_code)]

mod allocation;
mod segment_store;
mod state;

pub use state::GcHeap;

use core::{cell::Cell, ptr::NonNull};
use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

use crate::{
    batch::AllocationAccounting,
    budget::HeapBudget,
    error::{GcError, Result},
    header::{AllocationHeader, BlockKind, MarkState, NumericOwnership},
    ids::{GcObjectId, HeapId, RootToken, SegmentId},
    mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcSuspendGuard},
    object::{ObjectBlock, ObjectSlot, resolve_slot},
    registry,
    root::{RootKind, RootRegistry},
    scratch::ScratchArena,
    segment::SegmentKind,
    stats::HeapStats,
    trace::{ObjectGraph, Tracer},
};

use allocation::align_up;

/// Numeric bump 水位（微基准 `bump_ephemeral`：`clear_numeric_to` 整段 rewind）。
#[derive(Debug, Clone)]
pub struct NumericBumpMark {
    /// `(segment_slot, used, live_count)`，仅 Numeric 段。
    segments: Vec<(usize, usize, u32)>,
}

/// CAS runtime heap。

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
            drop_busy_leaks: Rc::new(Cell::new(0)),
            objects: Vec::new(),
            free_objects: Vec::new(),
            traced_numeric: HashSet::new(),
            bump_ephemeral: false,
            accounting: AllocationAccounting::Full,
            batch_bytes: 0,
            batch_allocs: 0,
            registered: false,
        }
    }

    /// 共享堆（登记 `HeapId`，供 numeric Drop 回找）。
    pub fn new_shared(budget: HeapBudget) -> Rc<RefCell<Self>> {
        let rc = Rc::new(RefCell::new(Self::new_inner(budget)));
        let leaks = rc.borrow().drop_busy_leaks.clone();
        let id = registry::register(&rc, leaks);
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
        s.drop_busy_leaks = self.drop_busy_leaks.get();
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

    /// 分配 numeric limb block（Rust `Drop` / `release_numeric_block` 负责释放）。
    pub fn allocate_numeric_block(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) = self.allocate_payload(
            SegmentKind::Numeric,
            BlockKind::Numeric,
            payload_bytes,
            u32::MAX,
            NumericOwnership::RustOwned,
        )?;
        Ok(NumericBlock { ptr: limbs.cast(), capacity: capacity_limbs, segment_id: seg_id, heap_id: self.id })
    }

    /// 分配 GC-owned numeric block（须经 root / Trace 保活；由 tracing sweep 回收）。
    ///
    /// 与 [`Self::allocate_numeric_block`] 互斥：本路径写入 [`NumericOwnership::GcOwned`]，
    /// Rust `Drop` / [`Self::release_numeric_block`] 不得 free。
    pub fn allocate_traced_numeric(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) = self.allocate_payload(
            SegmentKind::Numeric,
            BlockKind::Numeric,
            payload_bytes,
            u32::MAX,
            NumericOwnership::GcOwned,
        )?;
        self.traced_numeric.insert(limbs.as_ptr() as usize);
        Ok(NumericBlock { ptr: limbs.cast(), capacity: capacity_limbs, segment_id: seg_id, heap_id: self.id })
    }

    /// 为 GC-owned limbs 登记一条 [`NumericRoot`]（值对象持有 / 共享 `Clone`）。
    pub fn register_numeric_root(&mut self, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        let ownership = self.numeric_ownership(limbs)?;
        if ownership != NumericOwnership::GcOwned {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        Ok(self.roots.register_numeric(limbs.cast(), kind))
    }

    /// 撤掉一条指向该 payload 的 [`NumericRoot`]（Living `19`：Drop 只撤 root，不 free）。
    pub fn unregister_one_numeric_root(&mut self, limbs: NonNull<u64>) -> Result<()> {
        let ownership = self.numeric_ownership(limbs)?;
        if ownership != NumericOwnership::GcOwned {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        if !self.roots.unregister_one_numeric_for_payload(limbs.cast()) {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        Ok(())
    }

    /// 经 registry 登记 numeric root。
    pub fn register_numeric_root_registered(heap_id: HeapId, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        registry::with_heap(heap_id, |heap| heap.register_numeric_root(limbs, kind))?
    }

    /// 经 registry 撤一条 numeric root。
    pub fn unregister_one_numeric_root_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| heap.unregister_one_numeric_root(limbs))?
    }

    /// 将已初始化 limb 提升到长期 numeric segment（scratch → heap promote）。
    pub fn promote_limbs(&mut self, limbs: &[u64]) -> Result<NumericBlock> {
        let capacity = limbs.len().max(1);
        let block = self.allocate_traced_numeric(capacity)?;
        // SAFETY: 新 block 可写 capacity 个 limb。
        unsafe {
            if limbs.is_empty() {
                block.ptr.write(0);
            }
            else {
                core::ptr::copy_nonoverlapping(limbs.as_ptr(), block.ptr.as_ptr(), limbs.len());
            }
        }
        Ok(block)
    }

    /// 从 GC scratch 已初始化字节区提升为 limb block（`byte_len` 须为 8 的倍数）。
    pub fn promote_scratch_bytes(&mut self, start: usize, byte_len: usize) -> Result<NumericBlock> {
        if !byte_len.is_multiple_of(8) {
            return Err(GcError::InvalidCapacity);
        }
        let bytes = self.scratch.view_bytes(start, byte_len)?;
        let limbs_len = byte_len / 8;
        if limbs_len == 0 {
            return self.promote_limbs(&[]);
        }
        let mut tmp = Vec::with_capacity(limbs_len);
        for chunk in bytes.chunks_exact(8) {
            tmp.push(u64::from_le_bytes(chunk.try_into().expect("8 bytes")));
        }
        self.promote_limbs(&tmp)
    }

    /// 分配 object 槽 + payload，返回 [`GcObjectId`]。
    pub fn allocate_object(&mut self, payload_bytes: usize) -> Result<GcObjectId> {
        if payload_bytes == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let index = if let Some(i) = self.free_objects.pop() {
            i
        }
        else {
            let i = u32::try_from(self.objects.len()).map_err(|_| GcError::InvalidCapacity)?;
            self.objects.push(None);
            i
        };
        let generation =
            self.objects.get(index as usize).and_then(|s| s.as_ref().map(|o| o.generation.wrapping_add(1).max(1))).unwrap_or(1);
        let (seg_id, ptr) = self.allocate_payload(
            SegmentKind::LongLivedObject,
            BlockKind::Object,
            payload_bytes,
            index,
            NumericOwnership::Unspecified,
        )?;
        self.objects[index as usize] = Some(ObjectSlot {
            generation,
            mark: Cell::new(MarkState::White),
            block: Some(ObjectBlock { ptr, byte_len: payload_bytes, segment_id: seg_id }),
        });
        Ok(GcObjectId { index, generation })
    }

    /// 解析 object payload。
    pub fn object_payload_mut(&mut self, id: GcObjectId) -> Result<&mut [u8]> {
        let block = {
            let slot = resolve_slot(&self.objects, id)?;
            slot.block.ok_or(GcError::StaleObject { index: id.index, expected_generation: id.generation })?
        };
        // SAFETY: block 由本 heap 分配且 slot 仍存活。
        Ok(unsafe { core::slice::from_raw_parts_mut(block.ptr.as_ptr(), block.byte_len) })
    }

    /// 只读 payload。
    pub fn object_payload(&self, id: GcObjectId) -> Result<&[u8]> {
        let block = {
            let slot = resolve_slot(&self.objects, id)?;
            slot.block.ok_or(GcError::StaleObject { index: id.index, expected_generation: id.generation })?
        };
        Ok(unsafe { core::slice::from_raw_parts(block.ptr.as_ptr(), block.byte_len) })
    }

    /// 显式释放 object（推进 generation，可供 stale 检测）。
    pub fn release_object(&mut self, id: GcObjectId) -> Result<()> {
        let index = id.index as usize;
        let Some(Some(slot)) = self.objects.get(index)
        else {
            return Err(GcError::StaleObject { index: id.index, expected_generation: id.generation });
        };
        if slot.generation != id.generation || slot.block.is_none() {
            return Err(GcError::StaleObject { index: id.index, expected_generation: id.generation });
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
        let _ = self.header_for_limbs(block.ptr)?;
        Ok(unsafe { core::slice::from_raw_parts_mut(block.ptr.as_ptr(), block.capacity) })
    }

    /// Limb 只读视图。
    pub fn numeric_limbs(&self, block: &NumericBlock) -> Result<&[u64]> {
        let _ = self.header_for_limbs(block.ptr)?;
        Ok(unsafe { core::slice::from_raw_parts(block.ptr.as_ptr(), block.capacity) })
    }

    /// Header（limb 或 object payload 起点）。

    /// 兼容旧名。

    /// 标记 allocation 可达。

    /// 标记 limbs。

    /// 显式释放 numeric block（仅 [`NumericOwnership::RustOwned`]）。
    pub fn release_numeric_block(&mut self, block: NumericBlock) -> Result<()> {
        let ownership = self.numeric_ownership(block.ptr)?;
        if ownership != NumericOwnership::RustOwned {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        self.release_or_pool_numeric(block.ptr.cast())
    }

    /// 经 registry 释放（`OwnedLimbBuffer::Drop`）。
    ///
    /// - [`NumericOwnership::RustOwned`]：`release_or_pool_numeric`
    /// - [`NumericOwnership::GcOwned`]：撤一条 [`NumericRoot`]（不 free）
    pub fn release_numeric_limbs_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| match heap.numeric_ownership(limbs) {
            Ok(NumericOwnership::RustOwned) => heap.release_or_pool_numeric(limbs.cast()),
            Ok(NumericOwnership::GcOwned) => heap.unregister_one_numeric_root(limbs),
            Ok(NumericOwnership::Unspecified) => {
                heap.stats.lifecycle_mismatch = heap.stats.lifecycle_mismatch.saturating_add(1);
                Err(GcError::LifecycleMismatch)
            }
            Err(e) => Err(e),
        })?
    }

    /// 读取 numeric header 的生命周期标记。
    pub fn numeric_ownership(&self, limbs: NonNull<u64>) -> Result<NumericOwnership> {
        Ok(self.header_for_limbs(limbs)?.numeric_ownership)
    }

    /// 经 registry 读取生命周期标记。
    pub fn numeric_ownership_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<NumericOwnership> {
        registry::with_heap(heap_id, |heap| heap.numeric_ownership(limbs))?
    }

    /// GC-owned 持有者计数（`pin_state`；`0` / 哨兵表示不可用）。
    pub fn numeric_pin_state(&self, limbs: NonNull<u64>) -> Result<u16> {
        Ok(self.header_for_limbs(limbs)?.pin_state)
    }

    /// 经 registry 读取 `pin_state`。
    pub fn numeric_pin_state_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<u16> {
        registry::with_heap(heap_id, |heap| heap.numeric_pin_state(limbs))?
    }

    /// 微基准 bump+clear：开启后 Rust-owned numeric `Drop` 为空操作，须配合 [`Self::clear_numeric_to`]。
    ///
    /// 优先使用 [`Self::begin_numeric_batch`]（含 Batched 记账）。生产 CAS 路径禁止开启。
    pub fn enable_bump_ephemeral(&mut self, on: bool) {
        self.bump_ephemeral = on;
    }

    pub(crate) fn set_bump_ephemeral(&mut self, on: bool) {
        self.bump_ephemeral = on;
    }

    /// 是否处于微基准 bump+clear 模式。
    #[inline]
    pub fn bump_ephemeral(&self) -> bool {
        self.bump_ephemeral
    }

    /// 当前分配记账策略。
    #[inline]
    pub fn accounting(&self) -> AllocationAccounting {
        self.accounting
    }

    pub(crate) fn set_accounting(&mut self, accounting: AllocationAccounting) {
        self.accounting = accounting;
    }

    pub(crate) fn reset_batch_counters(&mut self) {
        self.batch_bytes = 0;
        self.batch_allocs = 0;
    }

    pub(crate) fn take_batch_bytes(&mut self) -> usize {
        core::mem::take(&mut self.batch_bytes)
    }

    pub(crate) fn take_batch_allocs(&mut self) -> u64 {
        core::mem::take(&mut self.batch_allocs)
    }

    pub(crate) fn flush_batch_accounting(&mut self, bytes: usize, count: u64) {
        if bytes == 0 && count == 0 {
            return;
        }
        self.controller.record_allocation(bytes);
        self.stats.allocation_count = self.stats.allocation_count.saturating_add(count);
        self.stats.total_arena_bytes_allocated = self.stats.total_arena_bytes_allocated.saturating_add(bytes);
    }

    /// 开始 numeric batch lease（单次 `&mut`、Batched 记账、批末 rewind）。
    pub fn begin_numeric_batch(&mut self) -> Result<crate::batch::NumericBatch<'_>> {
        crate::batch::begin_numeric_batch(self)
    }

    /// 在闭包内跑完整个 batch（自动 `finish`）。
    pub fn with_numeric_batch<R>(&mut self, f: impl FnOnce(&mut crate::batch::NumericBatch<'_>) -> R) -> Result<R> {
        let mut batch = self.begin_numeric_batch()?;
        let out = f(&mut batch);
        batch.finish()?;
        Ok(out)
    }

    /// Criterion：临时切换记账策略（须成对恢复；优先用 batch lease）。
    pub fn with_accounting<R>(&mut self, accounting: AllocationAccounting, f: impl FnOnce(&mut Self) -> R) -> R {
        let prev = self.accounting;
        self.accounting = accounting;
        let out = f(self);
        self.accounting = prev;
        out
    }

    /// Criterion raw bump：只推进 cursor，不写 header、不增 live_count、不记账。
    ///
    /// 仅 [`AllocationAccounting::Off`] + `bump_ephemeral`。结果不可当作 numeric block。
    pub fn bench_bump_raw_bytes(&mut self, bytes: usize) -> Result<NonNull<u8>> {
        if !self.bump_ephemeral || !matches!(self.accounting, AllocationAccounting::Off) {
            return Err(GcError::InvalidCapacity);
        }
        let need = bytes.max(1);
        let (seg_index, offset) = self.bump_allocate(SegmentKind::Numeric, need)?;
        let seg = self.segments[seg_index].as_mut().expect("segment");
        Ok(unsafe { NonNull::new_unchecked(seg.bytes.as_mut_ptr().add(offset)) })
    }

    /// 记录当前 Numeric bump 水位（操作数构造完成之后、热路径之前调用）。
    pub fn mark_numeric_bump(&self) -> NumericBumpMark {
        let mut segments = Vec::new();
        for (i, slot) in self.segments.iter().enumerate() {
            if let Some(seg) = slot {
                if seg.meta.kind == SegmentKind::Numeric {
                    segments.push((i, seg.meta.used, seg.meta.live_count));
                }
            }
        }
        NumericBumpMark { segments }
    }

    /// 将 Numeric bump rewind 到 `mark`（仅 `bump_ephemeral`）。
    ///
    /// 失效 mark 之后分配的全部 Rust-owned numeric 指针。不缩容 segment 容量。
    pub fn clear_numeric_to(&mut self, mark: NumericBumpMark) -> Result<()> {
        if !self.bump_ephemeral {
            return Err(GcError::InvalidCapacity);
        }
        let mut kept = HashSet::with_capacity(mark.segments.len());
        for (i, used, live) in mark.segments {
            kept.insert(i);
            let Some(Some(seg)) = self.segments.get_mut(i)
            else {
                continue;
            };
            if seg.meta.kind != SegmentKind::Numeric {
                continue;
            }
            debug_assert!(used <= seg.meta.used);
            debug_assert!(live <= seg.meta.live_count);
            seg.meta.used = used;
            seg.meta.live_count = live;
        }
        for (i, slot) in self.segments.iter_mut().enumerate() {
            if kept.contains(&i) {
                continue;
            }
            let Some(seg) = slot.as_mut()
            else {
                continue;
            };
            if seg.meta.kind != SegmentKind::Numeric {
                continue;
            }
            seg.meta.used = 0;
            seg.meta.live_count = 0;
        }
        Ok(())
    }

    /// 无 tracing 的 collect（只回收空 segment）。
    pub fn collect(&mut self) -> Result<CollectReport> {
        self.collect_traced(&crate::trace::EmptyObjectGraph)
    }

    /// Tracing collect：roots → ObjectGraph → mark → sweep object/numeric → reclaim 空 segment。
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

    /// 存活 segment。

    /// Resident 字节。

    /// Rust-owned numeric：ephemeral 下 Drop 为空；否则真正 release（可 reclaim）。
    fn release_or_pool_numeric(&mut self, payload: NonNull<u8>) -> Result<()> {
        if self.bump_ephemeral {
            // 空洞保留到 `clear_numeric_to`；避免每 op TLS registry 税。
            return Ok(());
        }
        self.release_payload(payload, BlockKind::Numeric)
    }

    fn clear_marks(&mut self) {
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

    fn sweep_unmarked_objects(&mut self) -> u64 {
        let mut swept = 0u64;
        let indices: Vec<usize> = (0..self.objects.len()).collect();
        for index in indices {
            let should_sweep =
                self.objects[index].as_ref().is_some_and(|s| s.block.is_some() && s.mark.get() == MarkState::White);
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

    /// 回收未标记的 GC-owned numeric block（Rust-owned 不在 `traced_numeric` 内）。
    fn sweep_unmarked_traced_numeric(&mut self) -> u64 {
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
            let ownership = unsafe { (*header).numeric_ownership };
            if ownership != NumericOwnership::GcOwned {
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

    fn mark_object_id(&mut self, id: GcObjectId, gray: &mut Vec<GcObjectId>) {
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
        if let Some(ptr) = NonNull::new(payload.cast_mut()) {
            let _ = self.heap.mark_payload(ptr);
        }
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

/// 图索引 / 属性域 payload（typed segment，非独立 allocator）。
#[derive(Debug, Clone, Copy)]
pub struct GraphDomainBlock {
    /// Payload 起点。
    pub ptr: NonNull<u8>,
    /// 字节长度。
    pub byte_len: usize,
    /// Segment。
    pub segment_id: SegmentId,
    /// Owner heap。
    pub heap_id: HeapId,
    /// 所属 typed domain。
    pub kind: SegmentKind,
}

impl GraphDomainBlock {
    /// 读取 `u64` 区间（不借 [`GcHeap`]，避免 `RefCell` 重入）。
    ///
    /// 调用方须保证 block 仍被 root / pin 保活。
    pub fn read_u64s(&self, offset_elems: usize, len: usize) -> Result<Vec<u64>> {
        if !matches!(self.kind, SegmentKind::GraphIndex | SegmentKind::GraphProperty) {
            return Err(GcError::InvalidCapacity);
        }
        let byte_off = offset_elems.checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let byte_len = len.checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let end = byte_off.checked_add(byte_len).ok_or(GcError::InvalidCapacity)?;
        if end > self.byte_len {
            return Err(GcError::InvalidCapacity);
        }
        let mut out = vec![0u64; len];
        // SAFETY: 调用方保证 payload 在 root/pin 下仍有效；bounds 已校验。
        unsafe {
            let src = self.ptr.as_ptr().add(byte_off).cast::<u64>();
            core::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), len);
        }
        Ok(out)
    }

    /// 写入 `u64` 区间（不借 [`GcHeap`]）。
    pub fn write_u64s(&self, offset_elems: usize, values: &[u64]) -> Result<()> {
        if !matches!(self.kind, SegmentKind::GraphIndex | SegmentKind::GraphProperty) {
            return Err(GcError::InvalidCapacity);
        }
        let byte_off = offset_elems.checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let byte_len = values.len().checked_mul(8).ok_or(GcError::InvalidCapacity)?;
        let end = byte_off.checked_add(byte_len).ok_or(GcError::InvalidCapacity)?;
        if end > self.byte_len {
            return Err(GcError::InvalidCapacity);
        }
        // SAFETY: 写路径仅在分配后或 pin 保护下使用；bounds 已校验。
        unsafe {
            let dst = self.ptr.as_ptr().add(byte_off).cast::<u64>();
            core::ptr::copy_nonoverlapping(values.as_ptr(), dst, values.len());
        }
        Ok(())
    }
}

impl GcHeap {
    /// 在 [`SegmentKind::GraphIndex`] 域分配 payload。
    pub fn allocate_graph_index(&mut self, payload_bytes: usize) -> Result<GraphDomainBlock> {
        self.allocate_graph_domain(SegmentKind::GraphIndex, BlockKind::GraphIndex, payload_bytes)
    }

    /// 在 [`SegmentKind::GraphProperty`] 域分配 payload。
    pub fn allocate_graph_property(&mut self, payload_bytes: usize) -> Result<GraphDomainBlock> {
        self.allocate_graph_domain(SegmentKind::GraphProperty, BlockKind::GraphProperty, payload_bytes)
    }

    /// 分配 GraphIndex 域并写入 `u64` 序列（空切片仍分配最小 8 字节槽）。
    pub fn allocate_graph_index_u64s(&mut self, values: &[u64]) -> Result<GraphDomainBlock> {
        let bytes = values.len().saturating_mul(8).max(8);
        let block = self.allocate_graph_index(bytes)?;
        if !values.is_empty() {
            self.write_graph_domain_u64s(&block, 0, values)?;
        }
        Ok(block)
    }

    /// 分配 GraphProperty 域并写入 `u64` 序列。
    pub fn allocate_graph_property_u64s(&mut self, values: &[u64]) -> Result<GraphDomainBlock> {
        let bytes = values.len().saturating_mul(8).max(8);
        let block = self.allocate_graph_property(bytes)?;
        if !values.is_empty() {
            self.write_graph_domain_u64s(&block, 0, values)?;
        }
        Ok(block)
    }

    /// 从 GraphIndex / GraphProperty block 读取 `u64` 区间（按元素下标）。
    pub fn read_graph_domain_u64s(&self, block: &GraphDomainBlock, offset_elems: usize, len: usize) -> Result<Vec<u64>> {
        self.ensure_graph_domain_block(block)?;
        block.read_u64s(offset_elems, len)
    }

    /// 向 GraphIndex / GraphProperty block 写入 `u64` 区间。
    pub fn write_graph_domain_u64s(&mut self, block: &GraphDomainBlock, offset_elems: usize, values: &[u64]) -> Result<()> {
        self.ensure_graph_domain_block(block)?;
        block.write_u64s(offset_elems, values)
    }

    fn allocate_graph_domain(
        &mut self,
        kind: SegmentKind,
        block_kind: BlockKind,
        payload_bytes: usize,
    ) -> Result<GraphDomainBlock> {
        if payload_bytes == 0 {
            return Err(GcError::InvalidCapacity);
        }
        let (segment_id, ptr) =
            self.allocate_payload(kind, block_kind, payload_bytes, u32::MAX, NumericOwnership::Unspecified)?;
        Ok(GraphDomainBlock { ptr, byte_len: payload_bytes, segment_id, heap_id: self.id, kind })
    }

    fn ensure_graph_domain_block(&self, block: &GraphDomainBlock) -> Result<()> {
        if block.heap_id != self.id {
            return Err(GcError::WrongHeap);
        }
        if !matches!(block.kind, SegmentKind::GraphIndex | SegmentKind::GraphProperty) {
            return Err(GcError::InvalidCapacity);
        }
        Ok(())
    }
}

unsafe impl Send for GraphDomainBlock {}

/// Collect 报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectReport {
    /// 有效 mode。
    pub mode: GcMode,
    /// 回收段数。
    pub segments_reclaimed: u64,
    /// Sweep 掉的 object 数。
    pub objects_swept: u64,
    /// Sweep 掉的 GC-owned numeric block 数。
    pub numeric_blocks_swept: u64,
    /// Resident。
    pub resident_bytes: usize,
    /// 耗时 ns。
    pub gc_time_ns: u64,
    /// 峰值 arena（报告用）。
    pub peak_arena_bytes: usize,
    /// 峰值 scratch（报告用）。
    pub peak_scratch_bytes: usize,
}

/// 由 limb 指针读取 header 中的 [`HeapId`]（不校验 heap 仍存活）。
pub fn heap_id_for_limbs(limbs: NonNull<u64>) -> HeapId {
    // SAFETY: 调用方保证 ptr 指向本运行时分配的 limb 区。
    unsafe {
        let header = limbs.as_ptr().cast::<u8>().sub(AllocationHeader::size()).cast::<AllocationHeader>();
        (*header).heap_id
    }
}

unsafe impl Send for NumericBlock {}
