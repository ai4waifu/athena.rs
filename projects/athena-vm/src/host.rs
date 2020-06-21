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

/// 扩展算子 ID（与 `athena_types::ExtensionOperatorId` 对齐的宿主约定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtensionOpId(pub u32);

/// 索引轴表 ID（lower 时登记的 `IndexSpec` 序列，由 host 持有）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexAxesId(pub u32);

/// 单次 host 调用结果（句柄级，无 TermStore 所有权）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostOutcome {
    /// 写入结果槽的值。
    Value(SlotValue),
    /// 保留未求值 / 残差（engine 映射 Coverage）。
    Residual(SlotValue),
    /// 软失败：保留 echo 值，并附诊断（Reference 记 Invalid；VM 可升硬失败）。
    SoftInvalid {
        /// 回显 / 残差槽值。
        value: SlotValue,
        /// 结构化诊断。
        diagnostic: Diagnostic,
    },
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

    /// 进入作用域帧，返回 [`SlotValue::Scope`] 深度句柄。
    fn enter_scope(&mut self, parent: Option<SlotValue>) -> Result<HostOutcome> {
        let _ = parent;
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "enter_scope_unimplemented"),
        ))
    }

    /// 退出与 `scope` 句柄匹配的作用域帧。
    fn exit_scope(&mut self, scope: SlotValue) -> Result<HostOutcome> {
        let _ = scope;
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "exit_scope_unimplemented"),
        ))
    }

    /// 由已求值元素构造类型化集合。
    fn construct_collection(
        &mut self,
        kind: athena_types::CollectionKind,
        args: &[SlotValue],
    ) -> Result<HostOutcome> {
        let _ = (kind, args);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "construct_collection_unimplemented"),
        ))
    }

    /// 对目标值应用已登记的索引轴规格。
    fn apply_index(&mut self, op: IndexAxesId, target: SlotValue) -> Result<HostOutcome> {
        let _ = (op, target);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "apply_index_unimplemented"),
        ))
    }

    /// 应用扩展算子（down-value 或残差）。
    fn apply_extension(&mut self, op: ExtensionOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = (op, args);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "apply_extension_unimplemented"),
        ))
    }

    /// 注册 pattern → replacement 分派规则。
    fn register_rule_dispatch(
        &mut self,
        head: SlotValue,
        operator: ExtensionOpId,
        pattern: SlotValue,
        replacement: SlotValue,
    ) -> Result<HostOutcome> {
        let _ = (head, operator, pattern, replacement);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "register_rule_dispatch_unimplemented"),
        ))
    }

    /// 挂接 Session 已编译规则。
    fn register_compiled_rule(&mut self, table: u32, rule: u32) -> Result<HostOutcome> {
        let _ = (table, rule);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                .detail("component", "VmHost")
                .detail("reason", "register_compiled_rule_unimplemented"),
        ))
    }
}

/// 拒绝一切语义 / provider 的空 host（骨架 / parity）。
#[derive(Debug, Default, Clone, Copy)]
pub struct NullHost;

impl VmHost for NullHost {}
