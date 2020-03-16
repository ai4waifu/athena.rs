//! 优化结果合同与空分派。

use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode};

use super::{
    certificate::BoundCertificate, fingerprint::OptimizationFingerprint, frontier::OptimizationFrontier, request::OptimizationRequest,
};

/// 优化领域结果。
///
/// 禁止只返回一个向量并默认称为最优。
/// `Feasible` ≠ `Optimal`；局部 KKT ≠ 全局；迭代收敛 ≠ 证明；
/// incumbent ≠ 整数全局最优；资源耗尽只能是 `ResourceLimited` / `Inconclusive`。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum OptimizationResult {
    /// 已证明最优（必须携带最优性种类，见证书）。
    Optimal {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态。
        status: ComputationStatus,
        /// 解点（变量赋值，骨架用 `f64` 向量占位）。
        point: Vec<f64>,
        /// 目标值。
        value: f64,
        /// 证书。
        certificate: BoundCertificate,
    },
    /// 可行但未证明最优。
    Feasible {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态。
        status: ComputationStatus,
        /// 可行点。
        point: Vec<f64>,
        /// 目标值。
        value: f64,
        /// 可选界证书。
        bound: Option<BoundCertificate>,
    },
    /// 不可行。
    Infeasible {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态。
        status: ComputationStatus,
        /// 不可行证书。
        certificate: BoundCertificate,
    },
    /// 无界。
    Unbounded {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态。
        status: ComputationStatus,
        /// 射线 / 下降方向（骨架占位）。
        ray_or_direction: Vec<f64>,
        /// 证书。
        certificate: BoundCertificate,
    },
    /// 结论不足（保留 incumbent / 界 / frontier）。
    Inconclusive {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态。
        status: ComputationStatus,
        /// incumbent 点。
        incumbent: Option<Vec<f64>>,
        /// 界。
        bounds: Option<BoundCertificate>,
        /// 可恢复前沿。
        frontier: OptimizationFrontier,
    },
    /// 资源截断。
    ResourceLimited {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态（应为 [`ComputationStatus::ResourceLimited`]）。
        status: ComputationStatus,
        /// incumbent 点。
        incumbent: Option<Vec<f64>>,
        /// 界。
        bounds: Option<BoundCertificate>,
        /// 可恢复前沿。
        frontier: OptimizationFrontier,
    },
    /// 近似数值候选（**不得**进入 exact M-Graph）。
    NumericalCandidate {
        /// 问题指纹。
        fingerprint: OptimizationFingerprint,
        /// 外层状态（应为 [`ComputationStatus::Candidate`]）。
        status: ComputationStatus,
        /// 候选点。
        point: Vec<f64>,
        /// 约束残差。
        residual: f64,
        /// 对偶 / 最优性 gap。
        gap: Option<f64>,
        /// 诊断。
        diagnostics: Vec<Diagnostic>,
    },
    /// 输入无效。
    InvalidInput {
        /// 诊断。
        reason: Diagnostic,
    },
    /// 尚未实现的请求。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

impl OptimizationResult {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Optimal { fingerprint, status, point, value, certificate } => Self::Optimal {
                fingerprint: *fingerprint,
                status: *status,
                point: point.clone(),
                value: *value,
                certificate: certificate.owning_copy(),
            },
            Self::Feasible { fingerprint, status, point, value, bound } => Self::Feasible {
                fingerprint: *fingerprint,
                status: *status,
                point: point.clone(),
                value: *value,
                bound: bound.as_ref().map(BoundCertificate::owning_copy),
            },
            Self::Infeasible { fingerprint, status, certificate } => Self::Infeasible {
                fingerprint: *fingerprint,
                status: *status,
                certificate: certificate.owning_copy(),
            },
            Self::Unbounded { fingerprint, status, ray_or_direction, certificate } => Self::Unbounded {
                fingerprint: *fingerprint,
                status: *status,
                ray_or_direction: ray_or_direction.clone(),
                certificate: certificate.owning_copy(),
            },
            Self::Inconclusive { fingerprint, status, incumbent, bounds, frontier } => Self::Inconclusive {
                fingerprint: *fingerprint,
                status: *status,
                incumbent: incumbent.clone(),
                bounds: bounds.as_ref().map(BoundCertificate::owning_copy),
                frontier: frontier.owning_copy(),
            },
            Self::ResourceLimited { fingerprint, status, incumbent, bounds, frontier } => Self::ResourceLimited {
                fingerprint: *fingerprint,
                status: *status,
                incumbent: incumbent.clone(),
                bounds: bounds.as_ref().map(BoundCertificate::owning_copy),
                frontier: frontier.owning_copy(),
            },
            Self::NumericalCandidate {
                fingerprint,
                status,
                point,
                residual,
                gap,
                diagnostics,
            } => Self::NumericalCandidate {
                fingerprint: *fingerprint,
                status: *status,
                point: point.clone(),
                residual: *residual,
                gap: *gap,
                diagnostics: diagnostics.clone(),
            },
            Self::InvalidInput { reason } => Self::InvalidInput { reason: reason.clone() },
            Self::Unevaluated { reason } => Self::Unevaluated { reason: reason.clone() },
        }
    }
}

/// 请求对应的操作名（审计 / 诊断）。
pub fn operation_name(request: &OptimizationRequest) -> &'static str {
    match request {
        OptimizationRequest::ValidateProblem { .. } => "validate_problem",
        OptimizationRequest::Solve { .. } => "solve",
        OptimizationRequest::VerifyCertificate { .. } => "verify_certificate",
        OptimizationRequest::Resume { .. } => "resume",
    }
}

/// 执行优化请求（骨架：一律 `Unevaluated`，算法未接入）。
pub fn execute_optimization(request: OptimizationRequest) -> OptimizationResult {
    let op = operation_name(&request);
    OptimizationResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "optimization")
            .detail("operation", op)
            .detail("note", "bootstrap_contract_only"),
    }
}
