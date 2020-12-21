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
    /// 共轭转置（当前元素域为实数时等于转置）。
    ConjugateTranspose {
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
    /// 逐元素除。
    ElementwiseDivide {
        /// 左（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 右（对象句柄或符号绑定）。
        rhs: MatrixOperand,
    },
    /// 逐元素幂。
    ElementwisePower {
        /// 底（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 指数（对象句柄或符号绑定）。
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
    /// 右除求解 `x B = A`（MATLAB `A/B` / mrdivide；不显式求逆）。
    RightSolve {
        /// 左端 `A`（对象句柄或符号绑定）。
        a: MatrixOperand,
        /// 右端系数 `B`（方阵；对象句柄或符号绑定）。
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
    /// 零空间基（精确路径）。
    NullSpace {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
        /// `true` → 列向量基（MATLAB `null`）；`false` → 行向量基（Mathematica `NullSpace`）。
        column_basis: bool,
    },
    /// 欧几里得 2-范数（精确完美平方）。
    Norm {
        /// 输入向量（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 条件数估计（机器路径；精确输入会先提升到 `f64`）。
    ConditionNumber {
        /// 输入方阵（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 下三角（含对角）。
    Tril {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 上三角（含对角）。
    Triu {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// Kronecker 积 `A ⊗ B`。
    Kronecker {
        /// 左（对象句柄或符号绑定）。
        lhs: MatrixOperand,
        /// 右（对象句柄或符号绑定）。
        rhs: MatrixOperand,
    },
    /// 对角阵判定（MATLAB `isdiag`）。
    IsDiagonal {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
    /// 三角阵判定（MATLAB `istril` / `istriu`）。
    IsTriangular {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
        /// `true` → 下三角；`false` → 上三角。
        lower: bool,
    },
    /// 对称判定（MATLAB `issymmetric`）。
    IsSymmetric {
        /// 输入（对象句柄或符号绑定）。
        matrix: MatrixOperand,
    },
}

impl LinearAlgebraRequest {
    /// Owning 复制（仅句柄）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Transpose { matrix } => Self::Transpose { matrix: *matrix },
            Self::ConjugateTranspose { matrix } => Self::ConjugateTranspose { matrix: *matrix },
            Self::Index { matrix, row, col } => Self::Index { matrix: *matrix, row: *row, col: *col },
            Self::MatMul { lhs, rhs } => Self::MatMul { lhs: *lhs, rhs: *rhs },
            Self::Hadamard { lhs, rhs } => Self::Hadamard { lhs: *lhs, rhs: *rhs },
            Self::ElementwiseDivide { lhs, rhs } => Self::ElementwiseDivide { lhs: *lhs, rhs: *rhs },
            Self::ElementwisePower { lhs, rhs } => Self::ElementwisePower { lhs: *lhs, rhs: *rhs },
            Self::Rank { matrix } => Self::Rank { matrix: *matrix },
            Self::Det { matrix } => Self::Det { matrix: *matrix },
            Self::Rref { matrix } => Self::Rref { matrix: *matrix },
            Self::Solve { a, b } => Self::Solve { a: *a, b: *b },
            Self::RightSolve { a, b } => Self::RightSolve { a: *a, b: *b },
            Self::Inverse { matrix } => Self::Inverse { matrix: *matrix },
            Self::Trace { matrix } => Self::Trace { matrix: *matrix },
            Self::Dot { lhs, rhs } => Self::Dot { lhs: *lhs, rhs: *rhs },
            Self::Cross { lhs, rhs } => Self::Cross { lhs: *lhs, rhs: *rhs },
            Self::NullSpace { matrix, column_basis } => Self::NullSpace { matrix: *matrix, column_basis: *column_basis },
            Self::Norm { matrix } => Self::Norm { matrix: *matrix },
            Self::ConditionNumber { matrix } => Self::ConditionNumber { matrix: *matrix },
            Self::Tril { matrix } => Self::Tril { matrix: *matrix },
            Self::Triu { matrix } => Self::Triu { matrix: *matrix },
            Self::Kronecker { lhs, rhs } => Self::Kronecker { lhs: *lhs, rhs: *rhs },
            Self::IsDiagonal { matrix } => Self::IsDiagonal { matrix: *matrix },
            Self::IsTriangular { matrix, lower } => Self::IsTriangular { matrix: *matrix, lower: *lower },
            Self::IsSymmetric { matrix } => Self::IsSymmetric { matrix: *matrix },
        }
    }
}
