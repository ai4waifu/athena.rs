//! 线性代数域结果与分派。

use athena_types::{Diagnostic, DiagnosticCode, SymbolId};

use super::{
    exact::{
        ExactDetResult, ExactNormResult, ExactRankResult, ExactRrefResult, ExactSolveResult, ExactTraceResult, det_bareiss, invert_exact,
        norm2_exact, nullspace_exact, rank_exact, rref_rational, solve_exact, trace_exact,
    },
    machine::{MachineSolveResult, rank_machine, solve_machine},
    matrix_result::MatrixResult,
    object_ref::{MatrixObjectStore, MatrixRef},
    ops::{cross, dot, hadamard, index_scalar, matmul, transpose},
    request::LinearAlgebraRequest,
    status::AlgorithmGuarantee,
    value::MatrixValue,
};

/// 默认机器主元阈值。
pub const DEFAULT_PIVOT_THRESHOLD: f64 = 1e-12;

/// 线性代数域值。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
///
/// 矩阵值载荷走 [`MatrixResult`]（Living 16），使 shape / element domain / guarantee /
/// residual / diagnostics 与矩阵值同一次计算一起旅行。
#[derive(Debug, PartialEq)]
pub enum LinearAlgebraValue {
    /// 矩阵（带 Living 16 域结果信封）。
    Matrix(MatrixResult),
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
    /// 精确矩阵迹。
    ExactTrace(ExactTraceResult),
    /// 精确欧几里得范数。
    ExactNorm(ExactNormResult),
    /// `Dot` 收缩结果（向量投影为平坦 List，标量为原子）。
    Dot(MatrixValue),
    /// 精确 RREF。
    ExactRref(ExactRrefResult),
    /// 精确求解。
    ExactSolve(ExactSolveResult),
    /// 机器求解。
    MachineSolve(MachineSolveResult),
}

impl LinearAlgebraValue {
    /// Wrap an owned matrix with default guarantee from its element parent.
    pub fn matrix_outcome(value: MatrixValue) -> Self {
        let guarantee = if value.parent().element.is_machine() {
            AlgorithmGuarantee::Approximate
        } else {
            AlgorithmGuarantee::Exact
        };
        Self::Matrix(MatrixResult::from_owned(value, guarantee))
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Matrix(m) => Self::Matrix(m.owning_copy()),
            Self::ExactRank(r) => Self::ExactRank(*r),
            Self::MachineRank { rank, guarantee } => Self::MachineRank { rank: *rank, guarantee: *guarantee },
            Self::ExactDet(r) => Self::ExactDet(r.owning_copy()),
            Self::ExactTrace(r) => Self::ExactTrace(r.owning_copy()),
            Self::ExactNorm(r) => Self::ExactNorm(r.owning_copy()),
            Self::Dot(m) => Self::Dot(m.owning_copy()),
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
        LinearAlgebraRequest::Inverse { .. } => "inverse",
        LinearAlgebraRequest::Trace { .. } => "trace",
        LinearAlgebraRequest::Dot { .. } => "dot",
        LinearAlgebraRequest::Cross { .. } => "cross",
        LinearAlgebraRequest::NullSpace { .. } => "nullspace",
        LinearAlgebraRequest::Norm { .. } => "norm",
    }
}

/// 执行线性代数请求（经 [`MatrixObjectStore`] 解析 [`MatrixRef`]）。
pub fn execute_linear_algebra(request: LinearAlgebraRequest, store: &MatrixObjectStore) -> LinearAlgebraResult {
    execute_linear_algebra_with_bindings(request, store, &|_| None)
}

/// 执行线性代数请求，并允许 [`MatrixOperand::Binding`] 经定义层解析。
pub fn execute_linear_algebra_with_bindings(
    request: LinearAlgebraRequest,
    store: &MatrixObjectStore,
    matrix_binding: &dyn Fn(SymbolId) -> Option<MatrixRef>,
) -> LinearAlgebraResult {
    match run(request, store, matrix_binding) {
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

fn run(
    request: LinearAlgebraRequest,
    store: &MatrixObjectStore,
    matrix_binding: &dyn Fn(SymbolId) -> Option<MatrixRef>,
) -> Result<LinearAlgebraValue, Diagnostic> {
    match request {
        LinearAlgebraRequest::Transpose { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            Ok(LinearAlgebraValue::matrix_outcome(transpose(&matrix)))
        }
        LinearAlgebraRequest::Index { matrix, row, col } => {
            let matrix = resolve(store, matrix)?;
            Ok(LinearAlgebraValue::matrix_outcome(index_scalar(&matrix, row, col)?))
        }
        LinearAlgebraRequest::MatMul { lhs, rhs } => {
            let lhs = lhs.resolve_value(store, matrix_binding)?;
            let rhs = rhs.resolve_value(store, matrix_binding)?;
            Ok(LinearAlgebraValue::matrix_outcome(matmul(&lhs, &rhs)?))
        }
        LinearAlgebraRequest::Hadamard { lhs, rhs } => {
            let lhs = lhs.resolve_value(store, matrix_binding)?;
            let rhs = rhs.resolve_value(store, matrix_binding)?;
            // 与 `Dot` 同表面：行/列向量结果投影为平坦 List（MATLAB `.*`）。
            Ok(LinearAlgebraValue::Dot(hadamard(&lhs, &rhs)?))
        }
        LinearAlgebraRequest::Rank { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                let (rank, guarantee) = rank_machine(&matrix, DEFAULT_PIVOT_THRESHOLD)?;
                Ok(LinearAlgebraValue::MachineRank { rank, guarantee })
            }
            else {
                Ok(LinearAlgebraValue::ExactRank(rank_exact(&matrix)?))
            }
        }
        LinearAlgebraRequest::Det { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_det_deferred_l2")
                    .detail("hint", "use exact parent or LU in later slice"));
            }
            Ok(LinearAlgebraValue::ExactDet(det_bareiss(&matrix)?))
        }
        LinearAlgebraRequest::Rref { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "rref_exact_only"));
            }
            Ok(LinearAlgebraValue::ExactRref(rref_rational(&matrix)?))
        }
        LinearAlgebraRequest::Solve { a, b } => {
            let a = a.resolve_value(store, matrix_binding)?;
            let b = b.resolve_value(store, matrix_binding)?;
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
        LinearAlgebraRequest::Inverse { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_inverse_deferred")
                    .detail("hint", "use exact parent"));
            }
            Ok(LinearAlgebraValue::matrix_outcome(invert_exact(&matrix)?))
        }
        LinearAlgebraRequest::Trace { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_trace_deferred")
                    .detail("hint", "use exact parent"));
            }
            Ok(LinearAlgebraValue::ExactTrace(trace_exact(&matrix)?))
        }
        LinearAlgebraRequest::Dot { lhs, rhs } => {
            let lhs = lhs.resolve_value(store, matrix_binding)?;
            let rhs = rhs.resolve_value(store, matrix_binding)?;
            Ok(LinearAlgebraValue::Dot(dot(&lhs, &rhs)?))
        }
        LinearAlgebraRequest::Cross { lhs, rhs } => {
            let lhs = lhs.resolve_value(store, matrix_binding)?;
            let rhs = rhs.resolve_value(store, matrix_binding)?;
            Ok(LinearAlgebraValue::Dot(cross(&lhs, &rhs)?))
        }
        LinearAlgebraRequest::NullSpace { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_nullspace_deferred")
                    .detail("hint", "use exact parent"));
            }
            Ok(LinearAlgebraValue::matrix_outcome(nullspace_exact(&matrix)?))
        }
        LinearAlgebraRequest::Norm { matrix } => {
            let matrix = matrix.resolve_value(store, matrix_binding)?;
            if matrix.parent().element.is_machine() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "machine_norm_deferred")
                    .detail("hint", "use exact parent"));
            }
            Ok(LinearAlgebraValue::ExactNorm(norm2_exact(&matrix)?))
        }
    }
}
