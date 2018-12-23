//! 显式 `IndexSpec` — 内核 0-based；方言 1-based 只在 lowering 层转换。

use athena_types::{Diagnostic, DiagnosticCode};

use super::shape::MatrixShape;

/// 轴上的范围说明（内核 0-based，半开区间）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxisRange {
    /// 整轴。
    All,
    /// `[start, end)`。
    Range {
        /// 起点（含）。
        start: u64,
        /// 终点（不含）。
        end: u64,
    },
}

impl AxisRange {
    /// 解析为具体下标列表。
    pub fn resolve(&self, len: u64) -> Result<Vec<u64>, Diagnostic> {
        match self {
            Self::All => Ok((0..len).collect()),
            Self::Range { start, end } => {
                if *start > *end || *end > len {
                    return Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                        .detail("start", start.to_string())
                        .detail("end", end.to_string())
                        .detail("len", len.to_string()));
                }
                Ok((*start..*end).collect())
            }
        }
    }

    /// 长度。
    pub fn len(&self, axis_len: u64) -> Result<u64, Diagnostic> {
        Ok(self.resolve(axis_len)?.len() as u64)
    }
}

/// 内核索引规格（禁止方言偷偷改基）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexSpec {
    /// 单元素 `(row, col)`，0-based。
    Scalar {
        /// 行。
        row: u64,
        /// 列。
        col: u64,
    },
    /// 子块切片。
    Slice {
        /// 行范围。
        rows: AxisRange,
        /// 列范围。
        cols: AxisRange,
    },
}

impl IndexSpec {
    /// 校验标量下标落在 shape 内。
    pub fn check_scalar(shape: MatrixShape, row: u64, col: u64) -> Result<(), Diagnostic> {
        if row >= shape.rows || col >= shape.cols {
            return Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("row", row.to_string())
                .detail("col", col.to_string())
                .detail("rows", shape.rows.to_string())
                .detail("cols", shape.cols.to_string()));
        }
        Ok(())
    }

    /// 切片结果 shape。
    pub fn slice_shape(&self, shape: MatrixShape) -> Result<MatrixShape, Diagnostic> {
        match self {
            Self::Scalar { .. } => Ok(MatrixShape::new(1, 1)),
            Self::Slice { rows, cols } => {
                let r = rows.len(shape.rows)?;
                let c = cols.len(shape.cols)?;
                Ok(MatrixShape::new(r, c))
            }
        }
    }
}

/// 方言来源（仅用于 lowering 审计，不进入算法语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectOrigin {
    /// Mathematica 方言。
    Mathematica,
    /// MATLAB 方言。
    Matlab,
}

/// 将方言 1-based 标量下标转为内核 [`IndexSpec::Scalar`]。
///
/// Mathematica 与 MATLAB 均使用 1-based 表面索引；canonical 结果必须一致。
pub fn lower_1based_scalar(origin: DialectOrigin, row_1: u64, col_1: u64) -> Result<IndexSpec, Diagnostic> {
    let _ = origin;
    if row_1 == 0 || col_1 == 0 {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
            .detail("reason", "dialect_1based_requires_positive")
            .detail("row", row_1.to_string())
            .detail("col", col_1.to_string()));
    }
    Ok(IndexSpec::Scalar { row: row_1 - 1, col: col_1 - 1 })
}

/// 将方言 1-based 半开切片 `[r1,r2]×[c1,c2]`（含端点）转为内核半开 [`AxisRange`]。
pub fn lower_1based_inclusive_slice(
    origin: DialectOrigin,
    row_start_1: u64,
    row_end_1: u64,
    col_start_1: u64,
    col_end_1: u64,
) -> Result<IndexSpec, Diagnostic> {
    let _ = origin;
    if row_start_1 == 0 || col_start_1 == 0 || row_end_1 < row_start_1 || col_end_1 < col_start_1 {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("reason", "bad_1based_slice"));
    }
    Ok(IndexSpec::Slice {
        rows: AxisRange::Range { start: row_start_1 - 1, end: row_end_1 },
        cols: AxisRange::Range { start: col_start_1 - 1, end: col_end_1 },
    })
}
