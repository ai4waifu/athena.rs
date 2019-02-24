//! Arena / scratch 字节与 limb 上界。

use crate::error::{GcError, Result};

/// 分配与回收预算（`GcMode::Disabled` 也必须遵守）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapBudget {
    /// Arena 常驻字节上限（含 header）。
    pub max_arena_bytes: usize,
    /// Segment 数量上限。
    pub max_segment_count: usize,
    /// 单次或累计 limb（u64）上限。
    pub max_limbs: usize,
    /// Scratch 字节上限。
    pub max_scratch_bytes: usize,
}

impl Default for HeapBudget {
    fn default() -> Self {
        Self {
            max_arena_bytes: 256 * 1024 * 1024,
            max_segment_count: 4096,
            max_limbs: 64 * 1024 * 1024,
            max_scratch_bytes: 64 * 1024 * 1024,
        }
    }
}

impl HeapBudget {
    /// Criterion / 长迭代微基准用：仍强制检查，但抬高上限。
    ///
    /// bump arena 在 `Disabled` 下不会因 Drop 回退指针；操作数常驻时 ephemeral 结果会打出空洞并推进
    /// `used`，默认 256 MiB 会在 Criterion 百万次迭代中触顶。微基准不得静默关掉预算，只抬天花板。
    pub fn for_microbench() -> Self {
        Self {
            max_arena_bytes: 16 * 1024 * 1024 * 1024,
            max_segment_count: 1_048_576,
            max_limbs: usize::MAX / 16,
            max_scratch_bytes: 1024 * 1024 * 1024,
        }
    }

    /// 检查新增 arena 字节后是否超限。
    pub fn check_arena_bytes(&self, current: usize, additional: usize) -> Result<()> {
        let total = current.checked_add(additional).ok_or(GcError::InvalidCapacity)?;
        if total > self.max_arena_bytes {
            return Err(GcError::ArenaBytesLimit { requested_total: total, limit: self.max_arena_bytes });
        }
        Ok(())
    }

    /// 检查 segment 数。
    pub fn check_segment_count(&self, count: usize) -> Result<()> {
        if count > self.max_segment_count {
            return Err(GcError::SegmentCountLimit { count, limit: self.max_segment_count });
        }
        Ok(())
    }

    /// 检查 limb 数。
    pub fn check_limbs(&self, limbs: usize) -> Result<()> {
        if limbs > self.max_limbs {
            return Err(GcError::LimbLimit { requested: limbs, limit: self.max_limbs });
        }
        Ok(())
    }

    /// 检查 scratch 字节。
    pub fn check_scratch_bytes(&self, current: usize, additional: usize) -> Result<()> {
        let total = current.checked_add(additional).ok_or(GcError::InvalidCapacity)?;
        if total > self.max_scratch_bytes {
            return Err(GcError::ScratchBytesLimit { requested_total: total, limit: self.max_scratch_bytes });
        }
        Ok(())
    }
}
