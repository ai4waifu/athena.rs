//! 显式 `IndexSpec` — 内核 0-based；1-based 转换是中性 helper，不携带方言身份。

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

/// 将 1-based 标量下标转为内核 [`IndexSpec::Scalar`]。
///
/// 方言表面 lowering 属于 SXO；Athena 只接收已转换或经此中性 helper 的规格。
pub fn scalar_index_from_one_based(row_1: u64, col_1: u64) -> Result<IndexSpec, Diagnostic> {
    if row_1 == 0 || col_1 == 0 {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
            .detail("reason", "one_based_requires_positive")
            .detail("row", row_1.to_string())
            .detail("col", col_1.to_string()));
    }
    Ok(IndexSpec::Scalar { row: row_1 - 1, col: col_1 - 1 })
}

/// 将 1-based 闭区间切片转为内核半开 [`AxisRange`]。
pub fn slice_index_from_one_based_inclusive(
    row_start_1: u64,
    row_end_1: u64,
    col_start_1: u64,
    col_end_1: u64,
) -> Result<IndexSpec, Diagnostic> {
    if row_start_1 == 0 || col_start_1 == 0 || row_end_1 < row_start_1 || col_end_1 < col_start_1 {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("reason", "bad_one_based_slice"));
    }
    Ok(IndexSpec::Slice {
        rows: AxisRange::Range { start: row_start_1 - 1, end: row_end_1 },
        cols: AxisRange::Range { start: col_start_1 - 1, end: col_end_1 },
    })
}
