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

unsafe impl Send for NumericBlock {}

impl GcHeap {
    fn reclaim_authority(&self, limbs: NonNull<u64>) -> Result<ReclaimAuthority> {
        Ok(self.header_for_limbs(limbs)?.reclaim_authority)
    }

    /// 分配临时数值 limb 块（[`ReclaimAuthority::ExplicitRelease`]：`Drop` / `release_numeric_block`）。
    pub fn allocate_numeric_block(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) =
            self.allocate_payload(SegmentKind::Numeric, BlockKind::Numeric, payload_bytes, u32::MAX, ReclaimAuthority::ExplicitRelease)?;
        Ok(NumericBlock { ptr: limbs.cast(), capacity: capacity_limbs, segment_id: seg_id, heap_id: self.id })
    }

    /// 分配已发布数值块（[`ReclaimAuthority::TracingSweep`]：须经根 / `Trace` 保活）。
    ///
    /// 与 [`Self::allocate_numeric_block`] 互斥：本路径禁止 `release_numeric_block` / Rust `Drop` free。
    pub fn allocate_traced_numeric(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        if capacity_limbs == 0 {
            return Err(GcError::InvalidCapacity);
        }
        self.budget.check_limbs(capacity_limbs)?;
        let payload_bytes = capacity_limbs.checked_mul(core::mem::size_of::<u64>()).ok_or(GcError::InvalidCapacity)?;
        let (seg_id, limbs) =
            self.allocate_payload(SegmentKind::Numeric, BlockKind::Numeric, payload_bytes, u32::MAX, ReclaimAuthority::TracingSweep)?;
        self.traced_numeric.insert(limbs.as_ptr() as usize);
        Ok(NumericBlock { ptr: limbs.cast(), capacity: capacity_limbs, segment_id: seg_id, heap_id: self.id })
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

    /// 为可 root 的 limbs 登记一条 [`NumericRoot`]。
    ///
    /// Living `24`：校验 block kind + reclaim authority（可 root 能力），不查询 ownership 实体。
    pub fn register_numeric_root(&mut self, limbs: NonNull<u64>, kind: RootKind) -> Result<RootToken> {
        if !self.may_root_numeric(limbs)? {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        Ok(self.roots.register_numeric(limbs.cast(), kind))
    }

    /// 撤掉一条指向该载荷的 [`NumericRoot`]（Living `19`：`Drop` 只撤根，不释放）。
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

    /// 显式释放数值块（仅 [`ReclaimAuthority::ExplicitRelease`]）。
    pub fn release_numeric_block(&mut self, block: NumericBlock) -> Result<()> {
        if !self.may_explicit_release_numeric(block.ptr)? {
            self.stats.lifecycle_mismatch = self.stats.lifecycle_mismatch.saturating_add(1);
            return Err(GcError::LifecycleMismatch);
        }
        self.release_or_pool_numeric(block.ptr.cast())
    }

    /// 经注册表释放（`OwnedLimbBuffer::Drop`）。
    ///
    /// - [`ReclaimAuthority::ExplicitRelease`]：走 `release_or_pool_numeric`
    /// - [`ReclaimAuthority::TracingSweep`]：撤一条 [`NumericRoot`]（不释放块）
    pub fn release_numeric_limbs_registered(heap_id: HeapId, limbs: NonNull<u64>) -> Result<()> {
        registry::with_heap(heap_id, |heap| match heap.reclaim_authority(limbs) {
            Ok(ReclaimAuthority::ExplicitRelease) => heap.release_or_pool_numeric(limbs.cast()),
            Ok(ReclaimAuthority::TracingSweep) => heap.unregister_one_numeric_root(limbs),
            Ok(ReclaimAuthority::Unspecified) => {
                heap.stats.lifecycle_mismatch = heap.stats.lifecycle_mismatch.saturating_add(1);
                Err(GcError::LifecycleMismatch)
            }
            Err(e) => Err(e),
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
