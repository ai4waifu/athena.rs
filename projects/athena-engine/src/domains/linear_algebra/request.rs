//! 线性代数强类型请求（Living `28`：输入为 [`MatrixRef`]）。

use super::object_ref::MatrixRef;

/// 线性代数域请求（禁止字符串算法名）。
#[derive(Debug, Clone, PartialEq)]
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
