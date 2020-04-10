//! 执行出口。

use athena_types::Diagnostic;

/// VM 执行结束原因。
///
/// 语义准入（AdmissionGate）与领域结果物化仍由 `athena-engine` 负责。
#[derive(Debug, Clone, PartialEq)]
pub enum VmExit {
    /// 正常返回（骨架阶段不携带 value ref）。
    Returned,
    /// Guard / Reject 显式拒绝（engine 映射为诊断或 DeclaredExit）。
    Rejected,
    /// 可恢复挂起（frontier / resume 由 engine 解释）。
    Suspended,
    /// 调用方取消。
    Cancelled,
    /// VM 层步数或资源预算耗尽。
    BudgetExceeded,
    /// 结构化诊断（非法模块、未知指令等）。
    Diagnostic(Diagnostic),
}
