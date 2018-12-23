//! 跨方言 lowering → 同一内核算子（canonical parity）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    index::{DialectOrigin, IndexSpec, lower_1based_scalar},
    request::LinearAlgebraRequest,
    value::MatrixValue,
};

/// 方言表面算子（仅 lowering；不进入算法语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DialectMatrixOp {
    /// Mathematica `Dot` / MATLAB `*`（`mtimes`）。
    MatMul,
    /// Mathematica 逐元素 `Times` on matrices / MATLAB `.*`。
    Hadamard,
    /// Mathematica / MATLAB `Transpose`。
    Transpose,
    /// Mathematica `Part` / MATLAB `()` 标量取元。
    IndexScalar,
    /// Mathematica `LinearSolve` / MATLAB `\` / `mldivide`。
    LinearSolve,
    /// Mathematica `Det` / MATLAB `det`。
    Det,
    /// Mathematica `MatrixRank` / MATLAB `rank`。
    Rank,
    /// Mathematica `RowReduce`。
    Rref,
}

/// 将方言算子 + 操作数 lowering 为内核 [`LinearAlgebraRequest`]。
pub fn lower_dialect_op(
    origin: DialectOrigin,
    op: DialectMatrixOp,
    args: DialectArgs,
) -> Result<LinearAlgebraRequest, Diagnostic> {
    let _ = origin;
    match (op, args) {
        (DialectMatrixOp::MatMul, DialectArgs::Binary { lhs, rhs }) => Ok(LinearAlgebraRequest::MatMul { lhs, rhs }),
        (DialectMatrixOp::Hadamard, DialectArgs::Binary { lhs, rhs }) => Ok(LinearAlgebraRequest::Hadamard { lhs, rhs }),
        (DialectMatrixOp::Transpose, DialectArgs::Unary { matrix }) => Ok(LinearAlgebraRequest::Transpose { matrix }),
        (DialectMatrixOp::IndexScalar, DialectArgs::Index { matrix, row_1based, col_1based }) => {
            let IndexSpec::Scalar { row, col } = lower_1based_scalar(origin, row_1based, col_1based)?
            else {
                unreachable!();
            };
            Ok(LinearAlgebraRequest::Index { matrix, row, col })
        }
        (DialectMatrixOp::LinearSolve, DialectArgs::Solve { a, b }) => Ok(LinearAlgebraRequest::Solve { a, b }),
        (DialectMatrixOp::Det, DialectArgs::Unary { matrix }) => Ok(LinearAlgebraRequest::Det { matrix }),
        (DialectMatrixOp::Rank, DialectArgs::Unary { matrix }) => Ok(LinearAlgebraRequest::Rank { matrix }),
        (DialectMatrixOp::Rref, DialectArgs::Unary { matrix }) => Ok(LinearAlgebraRequest::Rref { matrix }),
        _ => Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "dialect_args_mismatch")),
    }
}

/// Lowering 参数。
#[derive(Debug, Clone, PartialEq)]
pub enum DialectArgs {
    /// 一元。
    Unary {
        /// 矩阵。
        matrix: MatrixValue,
    },
    /// 二元。
    Binary {
        /// 左。
        lhs: MatrixValue,
        /// 右。
        rhs: MatrixValue,
    },
    /// 求解。
    Solve {
        /// 系数矩阵。
        a: MatrixValue,
        /// 右端。
        b: MatrixValue,
    },
    /// 1-based 标量索引。
    Index {
        /// 矩阵。
        matrix: MatrixValue,
        /// 方言行（1-based）。
        row_1based: u64,
        /// 方言列（1-based）。
        col_1based: u64,
    },
}

/// MATLAB `*` vs `.*` 必须在 lowering 分派到不同内核算子。
pub fn matlab_star_kind(elementwise: bool) -> DialectMatrixOp {
    if elementwise { DialectMatrixOp::Hadamard } else { DialectMatrixOp::MatMul }
}
