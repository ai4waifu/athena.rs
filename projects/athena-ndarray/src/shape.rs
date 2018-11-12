//! Shape、axis、chunk plan 与 memory budget。

use crate::ArrayError;

/// 轴索引（非负）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Axis(pub u32);

/// 经 checked `u64` 校验的逻辑 shape，独立于可寻址 RAM。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalShape {
    dimensions: Vec<u64>,
    elements: u64,
}

impl LogicalShape {
    /// 创建 shape；元素个数溢出时失败。
    pub fn new(dimensions: impl Into<Vec<u64>>) -> Result<Self, ArrayError> {
        let dimensions = dimensions.into();
        let elements = dimensions
            .iter()
            .try_fold(1u64, |n, &d| n.checked_mul(d).ok_or(ArrayError::ShapeOverflow))?;
        Ok(Self {
            dimensions,
            elements,
        })
    }

    /// 各维长度。
    pub fn dimensions(&self) -> &[u64] {
        &self.dimensions
    }

    /// 秩（维数）。
    pub fn rank(&self) -> usize {
        self.dimensions.len()
    }

    /// 逻辑元素个数。
    pub const fn element_count(&self) -> u64 {
        self.elements
    }
}

/// 单次执行允许驻留 / 暂存 / 外溢的预算合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudget {
    max_resident_bytes: usize,
    max_scratch_bytes: usize,
    max_spill_bytes: usize,
    max_open_chunks: usize,
}

impl MemoryBudget {
    /// 创建仅约束驻留字节的预算（scratch / spill 默认与驻留相同）。
    pub fn new(max_resident_bytes: usize) -> Result<Self, ArrayError> {
        Self::detailed(max_resident_bytes, max_resident_bytes, max_resident_bytes, 8)
    }

    /// 完整预算。
    pub fn detailed(
        max_resident_bytes: usize,
        max_scratch_bytes: usize,
        max_spill_bytes: usize,
        max_open_chunks: usize,
    ) -> Result<Self, ArrayError> {
        if max_resident_bytes == 0 || max_open_chunks == 0 {
            return Err(ArrayError::ZeroBudget);
        }
        Ok(Self {
            max_resident_bytes,
            max_scratch_bytes,
            max_spill_bytes,
            max_open_chunks,
        })
    }

    /// 驻留预算字节数。
    pub const fn bytes(self) -> usize {
        self.max_resident_bytes
    }

    /// Scratch 预算。
    pub const fn scratch_bytes(self) -> usize {
        self.max_scratch_bytes
    }

    /// Spill 预算。
    pub const fn spill_bytes(self) -> usize {
        self.max_spill_bytes
    }

    /// 同时打开的最大 chunk 数。
    pub const fn max_open_chunks(self) -> usize {
        self.max_open_chunks
    }
}

/// 有界分块执行计划（禁止隐式全量 materialization）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkPlan {
    /// 每个 chunk 的逻辑元素上限。
    pub max_elements: usize,
    /// 逻辑起点（元素索引）。
    pub start: u64,
    /// 逻辑终点（不含）。
    pub end: u64,
}

impl ChunkPlan {
    /// 在 `[start, end)` 上按 `max_elements` 规划。
    pub fn new(start: u64, end: u64, max_elements: usize) -> Result<Self, ArrayError> {
        if max_elements == 0 {
            return Err(ArrayError::ZeroBudget);
        }
        if start > end {
            return Err(ArrayError::OutOfBounds);
        }
        Ok(Self {
            max_elements,
            start,
            end,
        })
    }

    /// 逻辑区间长度。
    pub fn span(self) -> Result<u64, ArrayError> {
        self.end
            .checked_sub(self.start)
            .ok_or(ArrayError::RangeOverflow)
    }

    /// 需要的 chunk 个数。
    pub fn chunk_count(self) -> Result<u64, ArrayError> {
        let span = self.span()?;
        if span == 0 {
            return Ok(0);
        }
        let max = self.max_elements as u64;
        Ok(span.div_ceil(max))
    }
}
