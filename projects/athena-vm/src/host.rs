//! Host 回调边界：engine 综合体向 VM 提供语义 / provider，VM 不拥有它们。
//!
//! `VmHost` 是 `athena-vm` 解释循环调用上层的窄接口。实现方是 `athena-engine`
//!（或测试 double）。禁止把 M-Graph admission、方言解析或持久 payload 塞进本 trait 的默认实现。

use athena_types::{Diagnostic, Result};

use crate::slot::SlotValue;

/// 封闭语义算子 ID（与 `athena-ir::SemanticOperator` 数值对齐的宿主约定）。
///
/// VM 只传 opaque `u32`，避免 `athena-vm` 依赖 `athena-ir`。engine host 负责解码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticOpId(pub u32);

/// Provider 调用点 ID（module 内描述符下标或宿主注册表键）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderOpId(pub u32);

/// 单次 host 调用结果（句柄级，无 TermStore 所有权）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    /// 写入结果槽的值。
    Value(SlotValue),
    /// 保留未求值 / 残差（engine 映射 Coverage）。
    Residual(SlotValue),
    /// 硬失败诊断。
    Diagnostic(Diagnostic),
}

/// engine（或测试）实现的宿主回调。
///
/// VM 解释循环在遇到语义 / provider 边时调用；默认实现一律 unsupported，
/// 迫使真实 host 显式覆盖。
pub trait VmHost {
    /// 应用封闭语义算子（实参已是槽句柄）。
    fn apply_semantic(&mut self, op: SemanticOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = (op, args);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "apply_semantic_unimplemented"),
        ))
    }

    /// 类型化 provider 调用。
    fn call_provider(&mut self, op: ProviderOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = (op, args);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "call_provider_unimplemented"),
        ))
    }

    /// 读取 Session / 作用域绑定（键须为 Symbol 槽）。
    fn read_binding(&mut self, key: SlotValue) -> Result<HostOutcome> {
        let _ = key;
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "read_binding_unimplemented"),
        ))
    }

    /// 写入 Session / 作用域绑定。
    fn write_binding(
        &mut self,
        key: SlotValue,
        value: SlotValue,
        kind: athena_types::BindingKind,
        evaluation: athena_types::BindingEvaluationPolicy,
    ) -> Result<HostOutcome> {
        let _ = (key, value, kind, evaluation);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "write_binding_unimplemented"),
        ))
    }
}

/// 拒绝一切语义 / provider 的空 host（骨架 / parity）。
#[derive(Debug, Default, Clone, Copy)]
pub struct NullHost;

impl VmHost for NullHost {}
