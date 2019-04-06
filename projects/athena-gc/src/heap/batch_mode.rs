//! 批模式与微基准 bump 路径（非生产 CAS 主路径）。

use core::ptr::NonNull;
use std::collections::HashSet;

use crate::{
    batch::AllocationAccounting,
    error::{GcError, Result},
    segment::SegmentKind,
};

use super::{numeric::NumericBumpMark, state::GcHeap};

impl GcHeap {
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
}
