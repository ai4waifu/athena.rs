//! `MemoryBudget` 运行时账本（四轴 enforce）。

use crate::{ArrayError, MemoryBudget};

/// 预算使用计数（禁止只存 [`MemoryBudget`] 字段而不检查）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BudgetLedger {
    /// 已计入的驻留字节。
    pub used_resident: usize,
    /// 已计入的 scratch 字节。
    pub used_scratch: usize,
    /// 已计入的 spill 逻辑字节。
    pub used_spill: usize,
    /// 当前打开的 chunk / lease 数。
    pub open_chunks: usize,
}

impl BudgetLedger {
    /// 空账本。
    pub const fn new() -> Self {
        Self { used_resident: 0, used_scratch: 0, used_spill: 0, open_chunks: 0 }
    }

    /// 尝试占用驻留字节。
    pub fn acquire_resident(&mut self, budget: MemoryBudget, bytes: usize) -> Result<(), ArrayError> {
        let total = self.used_resident.checked_add(bytes).ok_or(ArrayError::RangeOverflow)?;
        if total > budget.bytes() {
            return Err(ArrayError::ResidentBudgetExceeded { requested_total: total, limit: budget.bytes() });
        }
        self.used_resident = total;
        Ok(())
    }

    /// 归还驻留字节。
    pub fn release_resident(&mut self, bytes: usize) {
        self.used_resident = self.used_resident.saturating_sub(bytes);
    }

    /// 尝试占用 scratch。
    pub fn acquire_scratch(&mut self, budget: MemoryBudget, bytes: usize) -> Result<(), ArrayError> {
        let total = self.used_scratch.checked_add(bytes).ok_or(ArrayError::RangeOverflow)?;
        if total > budget.scratch_bytes() {
            return Err(ArrayError::ScratchBudgetExceeded { requested_total: total, limit: budget.scratch_bytes() });
        }
        self.used_scratch = total;
        Ok(())
    }

    /// 归还 scratch。
    pub fn release_scratch(&mut self, bytes: usize) {
        self.used_scratch = self.used_scratch.saturating_sub(bytes);
    }

    /// 尝试占用 spill 额度。
    pub fn acquire_spill(&mut self, budget: MemoryBudget, bytes: usize) -> Result<(), ArrayError> {
        let total = self.used_spill.checked_add(bytes).ok_or(ArrayError::RangeOverflow)?;
        if total > budget.spill_bytes() {
            return Err(ArrayError::SpillBudgetExceeded { requested_total: total, limit: budget.spill_bytes() });
        }
        self.used_spill = total;
        Ok(())
    }

    /// 归还 spill 额度。
    pub fn release_spill(&mut self, bytes: usize) {
        self.used_spill = self.used_spill.saturating_sub(bytes);
    }

    /// 打开一个 chunk（计入 `max_open_chunks`）。
    pub fn open_chunk(&mut self, budget: MemoryBudget) -> Result<(), ArrayError> {
        let next = self.open_chunks.checked_add(1).ok_or(ArrayError::RangeOverflow)?;
        if next > budget.max_open_chunks() {
            return Err(ArrayError::OpenChunksExceeded { requested: next, limit: budget.max_open_chunks() });
        }
        self.open_chunks = next;
        Ok(())
    }

    /// 关闭一个 chunk。
    pub fn close_chunk(&mut self) {
        self.open_chunks = self.open_chunks.saturating_sub(1);
    }

    /// 校验单次读请求：元素字节 ≤ 驻留剩余，且不开整表后门。
    pub fn check_read_request(
        &self,
        budget: MemoryBudget,
        element_size: usize,
        element_count: usize,
    ) -> Result<usize, ArrayError> {
        if element_size == 0 {
            return Ok(0);
        }
        let bytes = element_count.checked_mul(element_size).ok_or(ArrayError::RangeOverflow)?;
        if bytes > budget.bytes() {
            return Err(ArrayError::BudgetExceeded { requested: element_count, max: budget.bytes() / element_size });
        }
        let remaining = budget.bytes().saturating_sub(self.used_resident);
        if bytes > remaining {
            return Err(ArrayError::ResidentBudgetExceeded {
                requested_total: self.used_resident.saturating_add(bytes),
                limit: budget.bytes(),
            });
        }
        if self.open_chunks >= budget.max_open_chunks() {
            return Err(ArrayError::OpenChunksExceeded {
                requested: self.open_chunks.saturating_add(1),
                limit: budget.max_open_chunks(),
            });
        }
        Ok(bytes)
    }
}

/// 有界 chunk 访问守卫：占用 resident + open_chunks，Drop 时归还。
#[derive(Debug)]
pub struct ChunkGuard<'a> {
    ledger: &'a mut BudgetLedger,
    bytes: usize,
}

impl<'a> ChunkGuard<'a> {
    /// 在账本上打开一次有界读（调用方随后 `read_range`）。
    pub fn acquire(
        ledger: &'a mut BudgetLedger,
        budget: MemoryBudget,
        element_size: usize,
        element_count: usize,
    ) -> Result<Self, ArrayError> {
        let bytes = ledger.check_read_request(budget, element_size, element_count)?;
        ledger.open_chunk(budget)?;
        // open_chunk 成功后再占 resident；失败路径已返回。
        if let Err(err) = ledger.acquire_resident(budget, bytes) {
            ledger.close_chunk();
            return Err(err);
        }
        Ok(Self { ledger, bytes })
    }

    /// 本守卫占用的驻留字节。
    pub const fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Drop for ChunkGuard<'_> {
    fn drop(&mut self) {
        self.ledger.release_resident(self.bytes);
        self.ledger.close_chunk();
    }
}
