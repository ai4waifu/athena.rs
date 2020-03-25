//! 数值域：分配、根、提升、释放与 reclaim 权限查询。

use core::ptr::NonNull;

use crate::{
    error::{GcError, Result},
    header::{BlockKind, ReclaimAuthority},
    ids::{HeapId, RootToken, SegmentId},
    registry,
    root::RootKind,
    segment::SegmentKind,
};

use super::state::GcHeap;

/// 数值 bump 水位（微基准 `bump_ephemeral`：`clear_numeric_to` 整段回退）。
#[derive(Debug, Clone)]
pub struct NumericBumpMark {
    /// `(段槽位, used, live_count)`，仅数值段。
    pub(super) segments: Vec<(usize, usize, u32)>,
}

/// 临时数值 limb 块（[`ReclaimAuthority::ExplicitRelease`]）。
///
/// 仅本句柄可交给 [`GcHeap::release_numeric_block`]。禁止与已发布块共用类型。
#[derive(Debug, Clone, Copy)]
pub struct TemporaryNumericBlock {
    /// Limb 起点。
    pub ptr: NonNull<u64>,
    /// Limb 容量。
    pub capacity: usize,
    /// 所属段。
    pub segment_id: SegmentId,
    /// 所属堆。
    pub heap_id: HeapId,
}

/// 已发布数值 limb 块（[`ReclaimAuthority::TracingSweep`]）。
///
/// 仅经 root / Trace 保活；最终回收由 tracing sweep 完成。不得显式 `release_numeric_block`。
#[derive(Debug, Clone, Copy)]
pub struct PublishedNumericBlock {
    /// Limb 起点。
    pub ptr: NonNull<u64>,
    /// Limb 容量。
    pub capacity: usize,
    /// 所属段。
    pub segment_id: SegmentId,
    /// 所属堆。
    pub heap_id: HeapId,
}

unsafe impl Send for TemporaryNumericBlock {}
unsafe impl Send for PublishedNumericBlock {}

impl GcHeap {
    /// 分配临时数值 limb 块（[`ReclaimAuthority::ExplicitRelease`]）。
    pub fn allocate_numeric_block(&mut self, capacity_limbs: usize) -> Result<TemporaryNumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) =
            self.allocate_payload(SegmentKind::Numeric, BlockKind::Numeric, payload_bytes, u32::MAX, ReclaimAuthority::ExplicitRelease)?;
        Ok(TemporaryNumericBlock { ptr: limbs.cast(), capacity: capacity_limbs, segment_id: seg_id, heap_id: self.id })
    }

    /// 分配已发布数值块（[`ReclaimAuthority::TracingSweep`]：须经根 / `Trace` 保活）。
    ///
    /// 与 [`Self::allocate_numeric_block`] 互斥：本路径禁止 [`Self::release_numeric_block`]。
    pub fn allocate_traced_numeric(&mut self, capacity_limbs: usize) -> Result<PublishedNumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) =
            self.allocate_payload(SegmentKind::Numeric, BlockKind::Numeric, payload_bytes, u32::MAX, ReclaimAuthority::TracingSweep)?;
        self.traced_numeric.insert(limbs.as_ptr() as usize);
        Ok(PublishedNumericBlock { ptr: limbs.cast(), capacity: capacity_limbs, segment_id: seg_id, heap_id: self.id })
    }

    /// 该 numeric block 是否允许显式 release / unique buffer reuse（临时 allocation）。
    pub fn may_explicit_release_numeric(&self, limbs: NonNull<u64>) -> Result<bool> {
        let header = self.header_for_limbs(limbs)?;
        Ok(header.block_kind == BlockKind::Numeric && header.reclaim_authority == ReclaimAuthority::ExplicitRelease)
    }

    /// 该 numeric block 是否可登记 root（已发布、tracing reclaim）。
    pub fn may_root_numeric(&self, limbs: NonNull<u64>) -> Result<bool> {
        let header = self.header_for_limbs(limbs)?;
        Ok(header.block_kind == BlockKind::Numeric && header.reclaim_authority == ReclaimAuthority::TracingSweep)
    }

    /// 经注册表查询 [`Self::may_explicit_release_numeric`]。
    pub fn may_explicit_release_numeric_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<bool> {
        registry::with_heap(heap_id, |heap| heap.may_explicit_release_numeric(limbs))?
    }

    /// 经注册表查询 [`Self::may_root_numeric`]。
    pub fn may_root_numeric_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<bool> {
        registry::with_heap(heap_id, |heap| heap.may_root_numeric(limbs))?
    }

    /// 指定 numeric payload 上的 root 条数。
    pub fn numeric_root_count(&self, limbs: NonNull<u64>) -> Result<usize> {
        let _ = self.header_for_limbs(limbs)?;
        Ok(self.roots.numeric_root_count_for_payload(limbs.cast()))
    }

    /// 经注册表查询 [`Self::numeric_root_count`]。
    pub fn numeric_root_count_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<usize> {
        registry::with_heap(heap_id, |heap| heap.numeric_root_count(limbs))?
    }

    /// 为可 root 的已发布块登记一条 [`NumericRoot`]。
    pub fn register_numeric_root(&mut self, block: &PublishedNumericBlock, kind: RootKind) -> Result<RootToken> {
        self.register_numeric_root_ptr(block.ptr, kind)
    }

    /// 为可 root 的 limbs 登记一条 [`NumericRoot`]（内部 / Drop 路径）。
    ///
    /// 校验 block kind + reclaim authority（可 root 能力），不查询 ownership 实体。
    pub fn register_numeric_root_ptr(&mut self, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        if !self.may_root_numeric(limbs)? {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        Ok(self.roots.register_numeric(limbs.cast(), kind))
    }

    /// 撤掉一条指向该载荷的 [`NumericRoot`]（`Drop` 只撤根，不释放）。
    pub fn unregister_one_numeric_root(&mut self, limbs: NonNull<u64>) -> Result<()> {
        if !self.may_root_numeric(limbs)? {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        if !self.roots.unregister_one_numeric_for_payload(limbs.cast()) {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        Ok(())
    }

    /// 经注册表登记数值根（裸指针路径；优先 [`Self::register_numeric_root`]）。
    pub fn register_numeric_root_registered(heap_id: HeapId, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        registry::with_heap(heap_id, |heap| heap.register_numeric_root_ptr(limbs, kind))?
    }

    /// 经注册表撤一条数值根。
    pub fn unregister_one_numeric_root_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| heap.unregister_one_numeric_root(limbs))?
    }

    /// 将已初始化 limb 提升到长期数值段（暂存区 → 已发布堆）。
    pub fn promote_limbs(&mut self, limbs: &[u64]) -> Result<PublishedNumericBlock> {
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

    /// 从暂存区已初始化字节区提升为已发布 limb 块（`byte_len` 须为 8 的倍数）。
    pub fn promote_scratch_bytes(&mut self, start: usize, byte_len: usize) -> Result<PublishedNumericBlock> {
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

    /// 临时块可写 limb 视图。
    pub fn temporary_limbs_mut(&mut self, block: &TemporaryNumericBlock) -> Result<&mut [u64]> {
        self.limbs_mut(block.ptr, block.capacity)
    }

    /// 临时块只读 limb 视图。
    pub fn temporary_limbs(&self, block: &TemporaryNumericBlock) -> Result<&[u64]> {
        self.limbs(block.ptr, block.capacity)
    }

    /// 已发布块可写 limb 视图（须已持有唯一可变 capability）。
    pub fn published_limbs_mut(&mut self, block: &PublishedNumericBlock) -> Result<&mut [u64]> {
        self.limbs_mut(block.ptr, block.capacity)
    }

    /// 已发布块只读 limb 视图。
    pub fn published_limbs(&self, block: &PublishedNumericBlock) -> Result<&[u64]> {
        self.limbs(block.ptr, block.capacity)
    }

    /// Limb 可写视图（兼容临时句柄；请优先 [`Self::temporary_limbs_mut`]）。
    pub fn numeric_limbs_mut(&mut self, block: &TemporaryNumericBlock) -> Result<&mut [u64]> {
        self.temporary_limbs_mut(block)
    }

    /// Limb 只读视图（兼容临时句柄；请优先 [`Self::temporary_limbs`]）。
    pub fn numeric_limbs(&self, block: &TemporaryNumericBlock) -> Result<&[u64]> {
        self.temporary_limbs(block)
    }

    fn limbs_mut(&mut self, ptr: NonNull<u64>, capacity: usize) -> Result<&mut [u64]> {
        let _ = self.header_for_limbs(ptr)?;
        Ok(unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), capacity) })
    }

    fn limbs(&self, ptr: NonNull<u64>, capacity: usize) -> Result<&[u64]> {
        let _ = self.header_for_limbs(ptr)?;
        Ok(unsafe { core::slice::from_raw_parts(ptr.as_ptr(), capacity) })
    }

    /// 显式释放临时数值块（仅接受 [`TemporaryNumericBlock`]）。
    pub fn release_numeric_block(&mut self, block: TemporaryNumericBlock) -> Result<()> {
        // 防御：句柄类型已约束，仍校验 header reclaim authority。
        if !self.may_explicit_release_numeric(block.ptr)? {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        self.release_or_pool_numeric(block.ptr.cast())
    }

    /// 经注册表显式释放临时数值块（仅 [`ReclaimAuthority::ExplicitRelease`]）。
    ///
    /// 禁止对裸 pointer 猜类别；已发布块请用 [`Self::unregister_one_numeric_root_registered`]。
    pub fn release_temporary_numeric_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| {
            if !heap.may_explicit_release_numeric(limbs)? {
                heap.stats.lifecycle_mismatch = heap.stats.lifecycle_mismatch.saturating_add(1);
                return Err(GcError::LifecycleMismatch);
            }
            heap.release_or_pool_numeric(limbs.cast())
        })?
    }

    /// GC 持有者计数（`pin_state`；`0` / 哨兵表示不可用）。
    pub fn numeric_pin_state(&self, limbs: NonNull<u64>) -> Result<u16> {
        Ok(self.header_for_limbs(limbs)?.pin_state)
    }

    /// 经注册表读取 `pin_state`。
    pub fn numeric_pin_state_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<u16> {
        registry::with_heap(heap_id, |heap| heap.numeric_pin_state(limbs))?
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
