//! Shape、layout、stride 不变量。

use athena_types::{Diagnostic, DiagnosticCode};

/// 矩阵逻辑 shape（允许空矩阵与零维边：`0×n`、`m×0`、`0×0`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatrixShape {
    /// 行数。
    pub rows: u64,
    /// 列数。
    pub cols: u64,
}

impl MatrixShape {
    /// 构造 shape。
    pub const fn new(rows: u64, cols: u64) -> Self {
        Self { rows, cols }
    }

    /// 元素个数（溢出时失败）。
    pub fn element_count(self) -> Result<usize, Diagnostic> {
        let n = self
            .rows
            .checked_mul(self.cols)
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "element_count_overflow"))?;
        usize::try_from(n).map_err(|_| Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "element_count_usize"))
    }

    /// 是否为空（至少一边为 0）。
    pub const fn is_empty(self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    /// 是否方阵。
    pub const fn is_square(self) -> bool {
        self.rows == self.cols
    }

    /// 转置 shape。
    pub const fn transpose(self) -> Self {
        Self { rows: self.cols, cols: self.rows }
    }

    /// 矩阵乘 shape 推断；内维不匹配则失败。
    pub fn matmul(self, rhs: Self) -> Result<Self, Diagnostic> {
        if self.cols != rhs.rows {
            return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch)
                .detail("op", "matmul")
                .detail("lhs_cols", self.cols.to_string())
                .detail("rhs_rows", rhs.rows.to_string()));
        }
        Ok(Self::new(self.rows, rhs.cols))
    }

    /// 逐元素乘 / 广播诊断：当前要求同 shape（不做隐式广播扩张）。
    pub fn hadamard(self, rhs: Self) -> Result<Self, Diagnostic> {
        if self != rhs {
            return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch)
                .detail("op", "hadamard")
                .detail("lhs", format!("{}x{}", self.rows, self.cols))
                .detail("rhs", format!("{}x{}", rhs.rows, rhs.cols)));
        }
        Ok(self)
    }
}

/// 存储主序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageOrder {
    /// 行主序。
    RowMajor,
    /// 列主序。
    ColumnMajor,
}

/// 物理 layout：主序 + 显式 stride（元素步长，可为负以表达转置视图）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layout {
    /// 存储主序（稠密自有缓冲时的规范序）。
    pub order: StorageOrder,
    /// 行方向元素步长。
    pub row_stride: i64,
    /// 列方向元素步长。
    pub col_stride: i64,
}

impl Layout {
    /// 给定 shape 的稠密行主序 layout。
    pub fn row_major(shape: MatrixShape) -> Result<Self, Diagnostic> {
        let cols = i64::try_from(shape.cols)
            .map_err(|_| Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "cols_i64"))?;
        Ok(Self { order: StorageOrder::RowMajor, row_stride: cols, col_stride: 1 })
    }

    /// 给定 shape 的稠密列主序 layout。
    pub fn column_major(shape: MatrixShape) -> Result<Self, Diagnostic> {
        let rows = i64::try_from(shape.rows)
            .map_err(|_| Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "rows_i64"))?;
        Ok(Self { order: StorageOrder::ColumnMajor, row_stride: 1, col_stride: rows })
    }

    /// 转置 layout（交换 stride，不立即物化）。
    pub const fn transposed(self) -> Self {
        Self {
            order: match self.order {
                StorageOrder::RowMajor => StorageOrder::ColumnMajor,
                StorageOrder::ColumnMajor => StorageOrder::RowMajor,
            },
            row_stride: self.col_stride,
            col_stride: self.row_stride,
        }
    }

    /// `(row, col)` → 线性偏移（相对缓冲起点）。
    pub fn offset(self, row: u64, col: u64) -> Result<isize, Diagnostic> {
        let r = i64::try_from(row).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidIndex))?;
        let c = i64::try_from(col).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidIndex))?;
        let off = r
            .checked_mul(self.row_stride)
            .and_then(|v| v.checked_add(c.checked_mul(self.col_stride)?))
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidIndex).detail("reason", "offset_overflow"))?;
        isize::try_from(off).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidIndex).detail("reason", "offset_isize"))
    }
}
