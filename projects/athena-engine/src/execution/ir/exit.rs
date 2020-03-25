//! 已声明的 guard / failure / deoptimization 出口。

use super::{
    ids::{BlockId, ExitId},
    types::ExecutionValueType,
};

/// 可能走出口边的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    /// Guard 谓词失败。
    GuardRejected,
    /// 缺少所需能力。
    CapabilityMissing,
    /// 预算耗尽 → 部分结果路径。
    BudgetExhausted,
    /// 已请求取消。
    Cancelled,
    /// 显式去优化到已声明的运行时出口（绝非旧 VM）。
    Deoptimize,
    /// Provider 返回了类型化的 unsupported / unknown。
    ProviderDiagnostic,
}

/// 一个已声明的 module 出口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredExit {
    /// 出口表下标。
    pub id: ExitId,
    /// 分类。
    pub kind: ExitKind,
    /// 可选的 module 内延续块。
    pub continuation: Option<BlockId>,
    /// 出口边上期望的值类型。
    pub result_types: Vec<ExecutionValueType>,
}

impl DeclaredExit {
    /// 无 module 内延续的 guard 拒绝。
    pub fn guard_reject(id: ExitId) -> Self {
        Self { id, kind: ExitKind::GuardRejected, continuation: None, result_types: Vec::new() }
    }
}
