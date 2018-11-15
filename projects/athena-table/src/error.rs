//! 表 / 查询结构化错误。

use athena_ndarray::ArrayError;

/// 表错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    /// 重复列名。
    DuplicateField(String),
    /// 列不是一维向量。
    NonVectorColumn,
    /// 行数不一致。
    RowCountMismatch {
        /// 期望。
        expected: u64,
        /// 实际。
        actual: u64,
    },
    /// 未知列。
    UnknownColumn(String),
    /// 下层 ndarray 错误。
    Array(ArrayError),
}

impl From<ArrayError> for TableError {
    fn from(value: ArrayError) -> Self {
        Self::Array(value)
    }
}
