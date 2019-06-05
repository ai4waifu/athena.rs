//! Numeric batch lease：单次 `&mut GcHeap` + 批次记账 + 批末 rewind。
//!
//! 生产路径保持 [`AllocationAccounting::Full`]。本模块供 Criterion / session 微基准使用，
//! **不得**泄漏为公共 CAS 默认语义。

use crate::{
    error::{GcError, Result},
    heap::{GcHeap, NumericBlock, NumericBumpMark},
};

/// 分配记账策略（绑定在 heap / batch lease 上，非进程全局开关）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationAccounting {
    /// 生产路径：逐次 stats + pressure + 可能 collect。
    #[default]
    Full,
    /// Batch lease：热路径只累加计数，[`NumericBatch::finish`] 一次性刷入。
    Batched,
    /// 仅 allocator / kernel microbench：跳过 stats 与 pressure（不代表生产语义）。
    Off,
}

/// 批次作用域：独占 `&mut GcHeap`，批内 bump，批末 rewind。
///
/// **合同**：
/// - 操作数须在 [`GcHeap::begin_numeric_batch`] 之前构造并保持存活。
/// - 批内结果不得逃逸；`finish` / Drop 后指针失效。
/// - 生产 CAS 不得默认进入本路径。
pub struct NumericBatch<'a> {
    heap: &'a mut GcHeap,
    mark: NumericBumpMark,
    prev_ephemeral: bool,
    prev_accounting: AllocationAccounting,
    finished: bool,
}

impl<'a> NumericBatch<'a> {
    pub(crate) fn new(heap: &'a mut GcHeap, mark: NumericBumpMark, prev_ephemeral: bool, prev_accounting: AllocationAccounting) -> Self {
        Self { heap, mark, prev_ephemeral, prev_accounting, finished: false }
    }

    /// 批内分配 numeric limb block（走当前 heap 的 `Batched`/`Off` 记账）。
    #[inline]
    pub fn allocate_limbs(&mut self, capacity_limbs: usize) -> Result<NumericBlock> {
        self.heap.allocate_numeric_block(capacity_limbs)
    }

    /// 底层 heap（批内独占可变借用）。
    #[inline]
    pub fn heap_mut(&mut self) -> &mut GcHeap {
        self.heap
    }

    /// 结束批次：rewind bump，并按需一次性刷入累计统计。
    pub fn finish(mut self) -> Result<()> {
        self.finish_inner()
    }

    fn finish_inner(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        let bytes = self.heap.take_batch_bytes();
        let count = self.heap.take_batch_allocs();
        self.heap.clear_numeric_to(self.mark.clone())?;
        self.heap.flush_batch_accounting(bytes, count);
        self.heap.set_bump_ephemeral(self.prev_ephemeral);
        self.heap.set_accounting(self.prev_accounting);
        Ok(())
    }
}

impl Drop for NumericBatch<'_> {
    fn drop(&mut self) {
        let _ = self.finish_inner();
    }
}

/// 开始 numeric batch（内部：打开 ephemeral + Batched 记账）。
pub(crate) fn begin_numeric_batch(heap: &mut GcHeap) -> Result<NumericBatch<'_>> {
    if matches!(heap.accounting(), AllocationAccounting::Batched) {
        return Err(GcError::InvalidCapacity);
    }
    let mark = heap.mark_numeric_bump();
    let prev_ephemeral = heap.bump_ephemeral();
    let prev_accounting = heap.accounting();
    heap.set_bump_ephemeral(true);
    heap.set_accounting(AllocationAccounting::Batched);
    heap.reset_batch_counters();
    Ok(NumericBatch::new(heap, mark, prev_ephemeral, prev_accounting))
}
