//! 线性代数强类型请求（输入为 [`MatrixRef`]）。

use super::object_ref::MatrixRef;

/// 线性代数域请求（禁止字符串算法名）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum LinearAlgebraRequest {
    /// 转置。
    Transpose {
        /// 输入。
        matrix: MatrixRef,
    },
    /// 标量索引（内核 0-based）。
    Index {
        /// 输入。
        matrix: MatrixRef,
        /// 行。
        row: u64,
        /// 列。
        col: u64,
    },
    /// 矩阵乘。
    MatMul {
        /// 左。
        lhs: MatrixRef,
        /// 右。
        rhs: MatrixRef,
    },
    /// 逐元素乘。
    Hadamard {
        /// 左。
        lhs: MatrixRef,
        /// 右。
        rhs: MatrixRef,
    },
    /// 秩（按元素 parent 分派精确/机器）。
    Rank {
        /// 输入。
        matrix: MatrixRef,
    },
    /// 行列式。
    Det {
        /// 输入。
        matrix: MatrixRef,
    },
    /// 行最简形（精确路径）。
    Rref {
        /// 输入。
        matrix: MatrixRef,
    },
    /// 线性求解 `A x = b`。
    Solve {
        /// 系数。
        a: MatrixRef,
        /// 右端 `m×1`。
        b: MatrixRef,
    },
}

impl LinearAlgebraRequest {
    /// Owning 复制（仅 `MatrixRef` 句柄）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Transpose { matrix } => Self::Transpose { matrix: *matrix },
            Self::Index { matrix, row, col } => Self::Index { matrix: *matrix, row: *row, col: *col },
            Self::MatMul { lhs, rhs } => Self::MatMul { lhs: *lhs, rhs: *rhs },
            Self::Hadamard { lhs, rhs } => Self::Hadamard { lhs: *lhs, rhs: *rhs },
            Self::Rank { matrix } => Self::Rank { matrix: *matrix },
            Self::Det { matrix } => Self::Det { matrix: *matrix },
            Self::Rref { matrix } => Self::Rref { matrix: *matrix },
            Self::Solve { a, b } => Self::Solve { a: *a, b: *b },
        }
    }
}
