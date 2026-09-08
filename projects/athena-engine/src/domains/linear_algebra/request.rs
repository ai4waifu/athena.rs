//! 线性代数强类型请求（输入为 [`MatrixRef`] / [`super::operand::MatrixOperand`]）。

use super::{object_ref::MatrixRef, operand::MatrixOperand};

/// 线性代数域请求（禁止字符串算法名）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum LinearAlgebraRequest {
    /// 转置。
    Transpose {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
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
        /// 左（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 右（对象句柄或符号绑定）。
        rhs: MatrixOperand,
    },
    /// 逐元素乘。
    Hadamard {
        /// 左（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 右（对象句柄或符号绑定）。
        rhs: MatrixOperand,
    },
    /// 秩（按元素 parent 分派精确/机器）。
    Rank {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 行列式。
    Det {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 行最简形（精确路径）。
    Rref {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 线性求解 `A x = b`。
    Solve {
        /// 系数（对象句柄或符号绑定）。
        a: MatrixOperand,
        /// 右端 `m×1`（对象句柄或符号绑定）。
        b: MatrixOperand,
    },
    /// 矩阵逆（精确路径）。
    Inverse {
        /// 输入方阵（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 矩阵迹（精确路径）。
    Trace {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 点积 / 矩阵乘收缩（Mathematica `Dot`）。
    Dot {
        /// 左（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 右（对象句柄或符号绑定）。
        rhs: MatrixOperand,
    },
    /// 三维叉积（Mathematica `Cross`）。
    Cross {
        /// 左（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 右（对象句柄或符号绑定）。
        rhs: MatrixOperand,
    },
    /// 零空间基（精确路径，行向量）。
    NullSpace {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 欧几里得 2-范数（精确完美平方）。
    Norm {
        /// 输入向量（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
}

impl LinearAlgebraRequest {
    /// Owning 复制（仅句柄）。
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
            Self::Inverse { matrix } => Self::Inverse { matrix: *matrix },
            Self::Trace { matrix } => Self::Trace { matrix: *matrix },
            Self::Dot { lhs, rhs } => Self::Dot { lhs: *lhs, rhs: *rhs },
            Self::Cross { lhs, rhs } => Self::Cross { lhs: *lhs, rhs: *rhs },
            Self::NullSpace { matrix } => Self::NullSpace { matrix: *matrix },
            Self::Norm { matrix } => Self::Norm { matrix: *matrix },
        }
    }
}
