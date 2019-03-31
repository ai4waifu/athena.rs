//! Layout · view · broadcast（绑 budget；view 不新分配 ArrayId）。

use crate::{ArrayError, Axis, LogicalShape, MemoryBudget};

/// 内存序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ArrayOrder {
    /// 行主序（最后一维连续）。
    #[default]
    RowMajor,
    /// 列主序（第一维连续）。
    ColumnMajor,
}

/// 物理/逻辑布局（≠ 数组身份）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLayout {
    /// 逻辑 shape。
    pub shape: LogicalShape,
    /// 各维元素步长（可为负，表示反向轴）。
    pub strides: Vec<i64>,
    /// 元素字节大小。
    pub item_size: usize,
    /// 默认序标签（自定义 strides 时仅作提示）。
    pub order: ArrayOrder,
}

impl ArrayLayout {
    /// 由 shape 生成紧凑 row-major strides。
    pub fn row_major(shape: LogicalShape, item_size: usize) -> Result<Self, ArrayError> {
        if item_size == 0 {
            return Err(ArrayError::BudgetTooSmall { element_size: 0 });
        }
        let dims = shape.dimensions();
        let mut strides = vec![0i64; dims.len()];
        let mut acc = 1i64;
        for (i, &d) in dims.iter().enumerate().rev() {
            strides[i] = acc;
            acc = acc.checked_mul(i64::try_from(d).map_err(|_| ArrayError::RangeOverflow)?).ok_or(ArrayError::RangeOverflow)?;
        }
        Ok(Self { shape, strides, item_size, order: ArrayOrder::RowMajor })
    }

    /// 由 shape 生成紧凑 column-major strides。
    pub fn column_major(shape: LogicalShape, item_size: usize) -> Result<Self, ArrayError> {
        if item_size == 0 {
            return Err(ArrayError::BudgetTooSmall { element_size: 0 });
        }
        let dims = shape.dimensions();
        let mut strides = vec![0i64; dims.len()];
        let mut acc = 1i64;
        for (i, &d) in dims.iter().enumerate() {
            strides[i] = acc;
            acc = acc.checked_mul(i64::try_from(d).map_err(|_| ArrayError::RangeOverflow)?).ok_or(ArrayError::RangeOverflow)?;
        }
        Ok(Self { shape, strides, item_size, order: ArrayOrder::ColumnMajor })
    }

    /// 校验 strides 长度与 shape 秩一致。
    pub fn validate(&self) -> Result<(), ArrayError> {
        if self.strides.len() != self.shape.rank() {
            return Err(ArrayError::LayoutMismatch);
        }
        if self.item_size == 0 {
            return Err(ArrayError::BudgetTooSmall { element_size: 0 });
        }
        Ok(())
    }

    /// 平坦元素下标 → 字节偏移（相对缓冲起点）。
    pub fn byte_offset_of_flat(&self, flat: u64) -> Result<usize, ArrayError> {
        self.validate()?;
        if flat >= self.shape.element_count() {
            return Err(ArrayError::OutOfBounds);
        }
        let coords = flat_to_coords(flat, self.shape.dimensions())?;
        let mut offset: i64 = 0;
        for (c, s) in coords.iter().zip(self.strides.iter()) {
            offset = offset
                .checked_add((*c as i64).checked_mul(*s).ok_or(ArrayError::RangeOverflow)?)
                .ok_or(ArrayError::RangeOverflow)?;
        }
        if offset < 0 {
            return Err(ArrayError::OutOfBounds);
        }
        let elem = usize::try_from(offset as u64).map_err(|_| ArrayError::RangeOverflow)?;
        elem.checked_mul(self.item_size).ok_or(ArrayError::RangeOverflow)
    }
}

fn flat_to_coords(mut flat: u64, dims: &[u64]) -> Result<Vec<u64>, ArrayError> {
    let mut coords = vec![0u64; dims.len()];
    for i in (0..dims.len()).rev() {
        let d = dims[i];
        if d == 0 {
            return Err(ArrayError::OutOfBounds);
        }
        coords[i] = flat % d;
        flat /= d;
    }
    Ok(coords)
}

/// 派生视图规格（不分配新逻辑数组身份；绑源 revision）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayViewSpec {
    /// 源数组 revision 号码（wire）。
    pub source_revision: u64,
    /// 视图逻辑 shape。
    pub shape: LogicalShape,
    /// 相对源缓冲的 strides。
    pub strides: Vec<i64>,
    /// 相对源起点的元素偏移。
    pub offset_elems: u64,
}

impl ArrayViewSpec {
    /// 恒等视图。
    pub fn identity(layout: &ArrayLayout, source_revision: u64) -> Result<Self, ArrayError> {
        layout.validate()?;
        Ok(Self { source_revision, shape: layout.shape.clone(), strides: layout.strides.clone(), offset_elems: 0 })
    }

    /// 源 revision 变更时失败。
    pub fn ensure_fresh(&self, current_revision: u64) -> Result<(), ArrayError> {
        if self.source_revision != current_revision {
            return Err(ArrayError::StaleView { expected: self.source_revision, actual: current_revision });
        }
        Ok(())
    }
}

/// 广播对齐结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcastSpec {
    /// 输出 shape。
    pub out_shape: LogicalShape,
    /// 左操作数各维扩展后的步长倍数（0 表示广播维）。
    pub left_stride_scale: Vec<u64>,
    /// 右操作数。
    pub right_stride_scale: Vec<u64>,
}

impl BroadcastSpec {
    /// NumPy 风格右对齐广播。
    pub fn align(left: &LogicalShape, right: &LogicalShape) -> Result<Self, ArrayError> {
        let ld = left.dimensions();
        let rd = right.dimensions();
        let rank = ld.len().max(rd.len());
        let mut out = vec![1u64; rank];
        let mut left_scale = vec![0u64; rank];
        let mut right_scale = vec![0u64; rank];
        for i in 0..rank {
            let li = if i < rank - ld.len() { 1 } else { ld[i - (rank - ld.len())] };
            let ri = if i < rank - rd.len() { 1 } else { rd[i - (rank - rd.len())] };
            let o = if li == ri {
                left_scale[i] = 1;
                right_scale[i] = 1;
                li
            }
            else if li == 1 {
                left_scale[i] = 0;
                right_scale[i] = 1;
                ri
            }
            else if ri == 1 {
                left_scale[i] = 1;
                right_scale[i] = 0;
                li
            }
            else {
                return Err(ArrayError::BroadcastIncompatible);
            };
            out[i] = o;
        }
        Ok(Self { out_shape: LogicalShape::new(out)?, left_stride_scale: left_scale, right_stride_scale: right_scale })
    }

    /// 流式二元逐元素：按预算分块访问输出平坦下标，禁止整表物化。
    pub fn for_each_flat_chunk(
        &self,
        budget: MemoryBudget,
        element_size: usize,
        mut visit: impl FnMut(u64, usize),
    ) -> Result<(), ArrayError> {
        if element_size == 0 {
            return Err(ArrayError::BudgetTooSmall { element_size: 0 });
        }
        let max = budget.bytes() / element_size;
        if max == 0 {
            return Err(ArrayError::BudgetTooSmall { element_size });
        }
        let n = self.out_shape.element_count();
        let mut offset = 0u64;
        while offset < n {
            let remaining = n - offset;
            let len = usize::try_from(remaining.min(max as u64)).unwrap_or(max);
            visit(offset, len);
            offset = offset.checked_add(len as u64).ok_or(ArrayError::RangeOverflow)?;
        }
        Ok(())
    }
}

/// 轴置换提示（用于 view；不改 ArrayId）。
pub fn permute_axes(shape: &LogicalShape, axes: &[Axis]) -> Result<LogicalShape, ArrayError> {
    if axes.len() != shape.rank() {
        return Err(ArrayError::LayoutMismatch);
    }
    let dims = shape.dimensions();
    let mut seen = vec![false; dims.len()];
    let mut out = Vec::with_capacity(dims.len());
    for Axis(a) in axes {
        let i = *a as usize;
        if i >= dims.len() || seen[i] {
            return Err(ArrayError::LayoutMismatch);
        }
        seen[i] = true;
        out.push(dims[i]);
    }
    LogicalShape::new(out)
}
