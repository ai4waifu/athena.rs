//! 线性代数域结果与分派。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    exact::{ExactDetResult, ExactRankResult, ExactRrefResult, ExactSolveResult, det_bareiss, rank_exact, rref_rational, solve_exact},
    machine::{MachineSolveResult, rank_machine, solve_machine},
    ops::{hadamard, index_scalar, matmul, transpose},
    request::LinearAlgebraRequest,
    status::AlgorithmGuarantee,
    value::MatrixValue,
};

/// 默认机器主元阈值。
pub const DEFAULT_PIVOT_THRESHOLD: f64 = 1e-12;

/// 线性代数域值。
#[derive(Debug, Clone, PartialEq)]
pub enum LinearAlgebraValue {
    /// 矩阵。
    Matrix(MatrixValue),
    /// 精确秩。
    ExactRank(ExactRankResult),
    /// 机器秩。
    MachineRank {
        /// 数值秩。
        rank: u64,
        /// 保证。
        guarantee: AlgorithmGuarantee,
    },
    /// 精确行列式。
    ExactDet(ExactDetResult),
    /// 精确 RREF。
    ExactRref(ExactRrefResult),
    /// 精确求解。
    ExactSolve(ExactSolveResult),
    /// 机器求解。
    MachineSolve(MachineSolveResult),
}

/// 线性代数结果。
#[derive(Debug, PartialEq)]
pub enum LinearAlgebraResult {
    /// 成功。
    Ok {
        /// 值。
        value: LinearAlgebraValue,
    },
    /// 诊断失败（shape/type 等）。
    Err {
        /// 诊断。
        diagnostic: Diagnostic,
    },
}

/// 操作名（审计）。
pub fn operation_name(request: &LinearAlgebraRequest) -> &'static str {
    match request {
        LinearAlgebraRequest::Transpose { .. } => "transpose",
        LinearAlgebraRequest::Index { .. } => "index",
        LinearAlgebraRequest::MatMul { .. } => "matmul",
        LinearAlgebraRequest::Hadamard { .. } => "hadamard",
        LinearAlgebraRequest::Rank { .. } => "rank",
        LinearAlgebraRequest::Det { .. } => "det",
        LinearAlgebraRequest::Rref { .. } => "rref",
        LinearAlgebraRequest::Solve { .. } => "solve",
    }
}

/// 执行线性代数请求。
pub fn execute_linear_algebra(request: LinearAlgebraRequest) -> LinearAlgebraResult {
    match run(request) {
        Ok(value) => LinearAlgebraResult::Ok { value },
        Err(diagnostic) => LinearAlgebraResult::Err { diagnostic },
    }
}

fn run(request: LinearAlgebraRequest) -> Result<LinearAlgebraValue, Diagnostic> {
    match request {
        LinearAlgebraRequest::Transpose { matrix } => Ok(LinearAlgebraValue::Matrix(transpose(&matrix))),
        LinearAlgebraRequest::Index { matrix, row, col } => Ok(LinearAlgebraValue::Matrix(index_scalar(&matrix, row, col)?)),
        LinearAlgebraRequest::MatMul { lhs, rhs } => Ok(LinearAlgebraValue::Matrix(matmul(&lhs, &rhs)?)),
        LinearAlgebraRequest::Hadamard { lhs, rhs } => Ok(LinearAlgebraValue::Matrix(hadamard(&lhs, &rhs)?)),
        LinearAlgebraRequest::Rank { matrix } => {
            if matrix.parent().element.is_machine() {
                let (rank, guarantee) = rank_machine(&matrix, DEFAULT_PIVOT_THRESHOLD)?;
                Ok(LinearAlgebraValue::MachineRank { rank, guarantee })
            }
            else {
                Ok(LinearAlgebraValue::ExactRank(rank_exact(&matrix)?))
            }
        }
        LinearAlgebraRequest::Det { matrix } => {
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_det_deferred_l2")
                    .detail("hint", "use exact parent or LU in later slice"));
            }
            Ok(LinearAlgebraValue::ExactDet(det_bareiss(&matrix)?))
        }
        LinearAlgebraRequest::Rref { matrix } => {
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "rref_exact_only"));
            }
            Ok(LinearAlgebraValue::ExactRref(rref_rational(&matrix)?))
        }
        LinearAlgebraRequest::Solve { a, b } => {
            if a.parent().element.is_machine() || b.parent().element.is_machine() {
                if !(a.parent().element.is_machine() && b.parent().element.is_machine()) {
                    return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "solve_parent_mixed"));
                }
                Ok(LinearAlgebraValue::MachineSolve(solve_machine(&a, &b, DEFAULT_PIVOT_THRESHOLD)?))
            }
            else {
                Ok(LinearAlgebraValue::ExactSolve(solve_exact(&a, &b)?))
            }
        }
    }
}
