//! `GcHeap`：分段、不移动的 bump 堆 + 对象区 + 追踪回收。
//!
//! Living `23`：`state` / `segment_store` / `allocation` / `collection` / `object` 已拆出；
//! 数值域、批模式与图域仍在本文件，后续批次迁出。
#![allow(unsafe_code)]

mod allocation;
mod collection;
mod object;
mod segment_store;
mod state;

pub use collection::CollectReport;
pub use state::GcHeap;

use core::{cell::Cell, ptr::NonNull};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::{
    batch::AllocationAccounting,
    budget::HeapBudget,
    error::{GcError, Result},
    header::{AllocationHeader, BlockKind, NumericOwnership},
    ids::{HeapId, RootToken, SegmentId},
    mode::{GcController, GcDeferGuard, GcMode, GcPinGuard, GcSuspendGuard},
    registry,
    root::{RootKind, RootRegistry},
    scratch::ScratchArena,
    segment::SegmentKind,
    stats::HeapStats,
};

/// 数值 bump 水位（微基准 `bump_ephemeral`：`clear_numeric_to` 整段回退）。
#[derive(Debug, Clone)]
pub struct NumericBumpMark {
    /// `(段槽位, used, live_count)`，仅数值段。
    segments: Vec<(usize, usize, u32)>,
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

    /// 根注册表。
    pub fn roots(&self) -> &RootRegistry {
        &self.roots
    }

    /// 根注册表（可变）。
    pub fn roots_mut(&mut self) -> &mut RootRegistry {
        &mut self.roots
    }

    /// 暂存区。
    pub fn scratch(&mut self) -> &mut ScratchArena {
        &mut self.scratch
    }

    /// 统计信息。
    pub fn stats(&self) -> HeapStats {
        let mut s = self.stats;
        s.peak_scratch_bytes = s.peak_scratch_bytes.max(self.scratch.peak_bytes());
        s.drop_busy_leaks = self.drop_busy_leaks.get();
        s
    }

    /// 当前有效 [`GcMode`]。
    pub fn effective_mode(&self) -> GcMode {
        self.controller.effective_mode()
    }

    /// 挂起自动回收。
    pub fn suspend(&self) -> GcSuspendGuard {
        self.controller.suspend()
    }

    /// 推迟回收到守卫结束。
    pub fn defer(&self) -> GcDeferGuard {
        self.controller.defer()
    }

    /// 钉住给定 segment，回收时跳过。
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

    /// 分配数值 limb 块（由 Rust `Drop` / `release_numeric_block` 负责释放）。
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

    /// 分配 GC 持有的数值块（须经根 / `Trace` 保活；由追踪清扫回收）。
    ///
    /// 与 [`Self::allocate_numeric_block`] 互斥：本路径写入 [`NumericOwnership::GcOwned`]，
    /// Rust `Drop` / [`Self::release_numeric_block`] 不得释放。
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

    /// 为 GC 持有的 limbs 登记一条 [`NumericRoot`]（值对象持有 / 共享 `Clone`）。
    pub fn register_numeric_root(&mut self, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        let ownership = self.numeric_ownership(limbs)?;
        if ownership != NumericOwnership::GcOwned {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        Ok(self.roots.register_numeric(limbs.cast(), kind))
    }

    /// 撤掉一条指向该载荷的 [`NumericRoot`]（Living `19`：`Drop` 只撤根，不释放）。
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

    /// 经注册表登记数值根。
    pub fn register_numeric_root_registered(heap_id: HeapId, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        registry::with_heap(heap_id, |heap| heap.register_numeric_root(limbs, kind))?
    }

    /// 经注册表撤一条数值根。
    pub fn unregister_one_numeric_root_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| heap.unregister_one_numeric_root(limbs))?
    }

    /// 将已初始化 limb 提升到长期数值段（暂存区 → 堆）。
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

    /// 从暂存区已初始化字节区提升为 limb 块（`byte_len` 须为 8 的倍数）。
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

    /// 显式释放数值块（仅 [`NumericOwnership::RustOwned`]）。
    pub fn release_numeric_block(&mut self, block: NumericBlock) -> Result<()> {
        let ownership = self.numeric_ownership(block.ptr)?;
        if ownership != NumericOwnership::RustOwned {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        self.release_or_pool_numeric(block.ptr.cast())
    }

    /// 经注册表释放（`OwnedLimbBuffer::Drop`）。
    ///
    /// - [`NumericOwnership::RustOwned`]：走 `release_or_pool_numeric`
    /// - [`NumericOwnership::GcOwned`]：撤一条 [`NumericRoot`]（不释放块）
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

    /// 读取数值块头的生命周期标记。
    pub fn numeric_ownership(&self, limbs: NonNull<u64>) -> Result<NumericOwnership> {
        Ok(self.header_for_limbs(limbs)?.numeric_ownership)
    }

    /// 经注册表读取生命周期标记。
    pub fn numeric_ownership_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<NumericOwnership> {
        registry::with_heap(heap_id, |heap| heap.numeric_ownership(limbs))?
    }

    /// GC 持有者计数（`pin_state`；`0` / 哨兵表示不可用）。
    pub fn numeric_pin_state(&self, limbs: NonNull<u64>) -> Result<u16> {
        Ok(self.header_for_limbs(limbs)?.pin_state)
    }

    /// 经注册表读取 `pin_state`。
    pub fn numeric_pin_state_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<u16> {
        registry::with_heap(heap_id, |heap| heap.numeric_pin_state(limbs))?
    }

    /// 微基准 bump+clear：开启后 Rust 持有数值块的 `Drop` 为空操作，须配合 [`Self::clear_numeric_to`]。
    ///
    /// 优先使用 [`Self::begin_numeric_batch`]（含批量记账）。生产 CAS 路径禁止开启。
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

    /// 开始数值批租约（单次 `&mut`、批量记账、批末回退水位）。
    pub fn begin_numeric_batch(&mut self) -> Result<crate::batch::NumericBatch<'_>> {
        crate::batch::begin_numeric_batch(self)
    }

    /// 在闭包内跑完整批（自动 `finish`）。
    pub fn with_numeric_batch<R>(&mut self, f: impl FnOnce(&mut crate::batch::NumericBatch<'_>) -> R) -> Result<R> {
        let mut batch = self.begin_numeric_batch()?;
        let out = f(&mut batch);
        batch.finish()?;
        Ok(out)
    }

    /// 基准用：临时切换记账策略（须成对恢复；优先用批租约）。
    pub fn with_accounting<R>(&mut self, accounting: AllocationAccounting, f: impl FnOnce(&mut Self) -> R) -> R {
        let prev = self.accounting;
        self.accounting = accounting;
        let out = f(self);
        self.accounting = prev;
        out
    }

    /// 基准用裸 bump：只推进游标，不写头、不增 `live_count`、不记账。
    ///
    /// 仅 [`AllocationAccounting::Off`] + `bump_ephemeral`。结果不可当作数值块。
    pub fn bench_bump_raw_bytes(&mut self, bytes: usize) -> Result<NonNull<u8>> {
        if !self.bump_ephemeral || !matches!(self.accounting, AllocationAccounting::Off) {
            return Err(GcError::InvalidCapacity);
        }
        let need = bytes.max(1);
        let (seg_index, offset) = self.bump_allocate(SegmentKind::Numeric, need)?;
        let seg = self.segments[seg_index].as_mut().expect("segment");
        Ok(unsafe { NonNull::new_unchecked(seg.bytes.as_mut_ptr().add(offset)) })
    }

    /// 记录当前数值 bump 水位（操作数构造完成之后、热路径之前调用）。
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

    /// 将数值 bump 回退到 `mark`（仅 `bump_ephemeral`）。
    ///
    /// 使 mark 之后分配的全部 Rust 持有数值指针失效。不缩容段容量。
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



    /// Rust 持有数值块：瞬时模式下 `Drop` 为空；否则真正释放（可再回收段）。
    fn release_or_pool_numeric(&mut self, payload: NonNull<u8>) -> Result<()> {
        if self.bump_ephemeral {
            // 空洞保留到 `clear_numeric_to`，避免每步操作走 TLS 注册表。
            return Ok(());
        }
        self.release_payload(payload, BlockKind::Numeric)
    }




}

impl Drop for GcHeap {
    fn drop(&mut self) {
        if self.registered {
            registry::unregister(self.id);
        }
    }
}



/// 数值 limb 块。
#[derive(Debug, Clone, Copy)]
pub struct NumericBlock {
    /// Limb 起点。
    pub ptr: NonNull<u64>,
    /// Limb 容量。
    pub capacity: usize,
    /// 所属段。
    pub segment_id: SegmentId,
    /// 所属堆。
    pub heap_id: HeapId,
}

/// 图索引 / 属性域载荷（类型化段，非独立分配器）。
#[derive(Debug, Clone, Copy)]
pub struct GraphDomainBlock {
    /// 载荷起点。
    pub ptr: NonNull<u8>,
    /// 字节长度。
    pub byte_len: usize,
    /// 所属段。
    pub segment_id: SegmentId,
    /// 所属堆。
    pub heap_id: HeapId,
    /// 所属类型化域。
    pub kind: SegmentKind,
}

impl GraphDomainBlock {
    /// 读取 `u64` 区间（不借 [`GcHeap`]，避免 `RefCell` 重入）。
    ///
    /// 调用方须保证块仍被根 / 钉住保活。
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
        // SAFETY: 调用方保证载荷在根/钉住下仍有效；边界已校验。
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
    /// 在 [`SegmentKind::GraphIndex`] 域分配载荷。
    pub fn allocate_graph_index(&mut self, payload_bytes: usize) -> Result<GraphDomainBlock> {
        self.allocate_graph_domain(SegmentKind::GraphIndex, BlockKind::GraphIndex, payload_bytes)
    }

    /// 在 [`SegmentKind::GraphProperty`] 域分配载荷。
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

    /// 从 GraphIndex / GraphProperty 块读取 `u64` 区间（按元素下标）。
    pub fn read_graph_domain_u64s(&self, block: &GraphDomainBlock, offset_elems: usize, len: usize) -> Result<Vec<u64>> {
        self.ensure_graph_domain_block(block)?;
        block.read_u64s(offset_elems, len)
    }

    /// 向 GraphIndex / GraphProperty 块写入 `u64` 区间。
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


/// 由 limb 指针读取头中的 [`HeapId`]（不校验堆是否仍存活）。
pub fn heap_id_for_limbs(limbs: NonNull<u64>) -> HeapId {
    // SAFETY: 调用方保证 ptr 指向本运行时分配的 limb 区。
    unsafe {
        let header = limbs.as_ptr().cast::<u8>().sub(AllocationHeader::size()).cast::<AllocationHeader>();
        (*header).heap_id
    }
}

unsafe impl Send for NumericBlock {}
