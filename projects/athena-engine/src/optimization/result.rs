//! 优化结果合同与空分派。

use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode};

use super::{
    certificate::BoundCertificate, fingerprint::OptimizationFingerprint, frontier::OptimizationFrontier,
    request::OptimizationRequest,
};

/// 优化领域结果。
///
/// 禁止只返回一个向量并默认称为最优。
/// `Feasible` ≠ `Optimal`；局部 KKT ≠ 全局；迭代收敛 ≠ 证明；
/// incumbent ≠ 整数全局最优；资源耗尽只能是 `ResourceLimited` / `Inconclusive`。
#[derive(Debug, Clone, PartialEq)]
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
