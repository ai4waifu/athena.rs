//! 线性代数域结果与分派。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    exact::{ExactDetResult, ExactRankResult, ExactRrefResult, ExactSolveResult, det_bareiss, rank_exact, rref_rational, solve_exact},
    machine::{MachineSolveResult, rank_machine, solve_machine},
    object_ref::{MatrixObjectStore, MatrixRef},
    ops::{hadamard, index_scalar, matmul, transpose},
    request::LinearAlgebraRequest,
    status::AlgorithmGuarantee,
    value::MatrixValue,
};

/// 默认机器主元阈值。
pub const DEFAULT_PIVOT_THRESHOLD: f64 = 1e-12;

/// 线性代数域值。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
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

impl LinearAlgebraValue {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Matrix(m) => Self::Matrix(m.owning_copy()),
            Self::ExactRank(r) => Self::ExactRank(*r),
            Self::MachineRank { rank, guarantee } => Self::MachineRank { rank: *rank, guarantee: *guarantee },
            Self::ExactDet(r) => Self::ExactDet(r.owning_copy()),
            Self::ExactRref(r) => Self::ExactRref(r.owning_copy()),
            Self::ExactSolve(r) => Self::ExactSolve(r.owning_copy()),
            Self::MachineSolve(r) => Self::MachineSolve(r.owning_copy()),
        }
    }
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

/// 执行线性代数请求（经 [`MatrixObjectStore`] 解析 [`MatrixRef`]）。
pub fn execute_linear_algebra(request: LinearAlgebraRequest, store: &MatrixObjectStore) -> LinearAlgebraResult {
    match run(request, store) {
        Ok(value) => LinearAlgebraResult::Ok { value },
        Err(diagnostic) => LinearAlgebraResult::Err { diagnostic },
    }
}

fn resolve(store: &MatrixObjectStore, r: MatrixRef) -> Result<MatrixValue, Diagnostic> {
    store.resolve_owning(r).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "linear_algebra")
            .detail("reason", "missing_matrix_ref")
            .arg("ref", r.0)
    })
}

fn run(request: LinearAlgebraRequest, store: &MatrixObjectStore) -> Result<LinearAlgebraValue, Diagnostic> {
    match request {
        LinearAlgebraRequest::Transpose { matrix } => {
            let matrix = resolve(store, matrix)?;
            Ok(LinearAlgebraValue::Matrix(transpose(&matrix)))
        }
        LinearAlgebraRequest::Index { matrix, row, col } => {
            let matrix = resolve(store, matrix)?;
            Ok(LinearAlgebraValue::Matrix(index_scalar(&matrix, row, col)?))
        }
        LinearAlgebraRequest::MatMul { lhs, rhs } => {
            let lhs = resolve(store, lhs)?;
            let rhs = resolve(store, rhs)?;
            Ok(LinearAlgebraValue::Matrix(matmul(&lhs, &rhs)?))
        }
        LinearAlgebraRequest::Hadamard { lhs, rhs } => {
            let lhs = resolve(store, lhs)?;
            let rhs = resolve(store, rhs)?;
            Ok(LinearAlgebraValue::Matrix(hadamard(&lhs, &rhs)?))
        }
        LinearAlgebraRequest::Rank { matrix } => {
            let matrix = resolve(store, matrix)?;
            if matrix.parent().element.is_machine() {
                let (rank, guarantee) = rank_machine(&matrix, DEFAULT_PIVOT_THRESHOLD)?;
                Ok(LinearAlgebraValue::MachineRank { rank, guarantee })
            }
            else {
                Ok(LinearAlgebraValue::ExactRank(rank_exact(&matrix)?))
            }
        }
        LinearAlgebraRequest::Det { matrix } => {
            let matrix = resolve(store, matrix)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_det_deferred_l2")
                    .detail("hint", "use exact parent or LU in later slice"));
            }
            Ok(LinearAlgebraValue::ExactDet(det_bareiss(&matrix)?))
        }
        LinearAlgebraRequest::Rref { matrix } => {
            let matrix = resolve(store, matrix)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "rref_exact_only"));
            }
            Ok(LinearAlgebraValue::ExactRref(rref_rational(&matrix)?))
        }
        LinearAlgebraRequest::Solve { a, b } => {
            let a = resolve(store, a)?;
            let b = resolve(store, b)?;
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
