//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期只覆盖最小句柄级语义（Boolean `Not`），完整 SSA 语义仍在
//! [`crate::execution::reference`]。终态由 Reference 循环迁入 VM 后扩展本 host。

use athena_ir::SemanticOperator;
use athena_types::{Diagnostic, DiagnosticCode, Result};
use athena_vm::{HostOutcome, ProviderOpId, SemanticOpId, SlotValue, VmHost};

/// 执行宿主（engine 在 VM 之上 · 不拥有解释循环）。
#[derive(Debug, Default, Clone, Copy)]
pub struct ExecutionHost;

impl ExecutionHost {
    /// 构造。
    pub const fn new() -> Self {
        Self
    }
}

impl VmHost for ExecutionHost {
    fn apply_semantic(&mut self, op: SemanticOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        // `SemanticOperator::Not` discriminant == 19（athena-ir 稳定编号）。
        if op.0 == SemanticOperator::Not.discriminant() {
            let Some(SlotValue::Boolean(v)) = args.first().copied() else {
                return Ok(HostOutcome::Diagnostic(
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionHost")
                        .detail("reason", "not_expects_boolean"),
                ));
            };
            return Ok(HostOutcome::Value(SlotValue::Boolean(!v)));
        }
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "apply_semantic_deferred_to_reference")
                .detail("op", op.0),
        ))
    }

    fn call_provider(&mut self, op: ProviderOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = (op, args);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "call_provider_deferred_to_reference")
                .detail("op", op.0),
        ))
    }
}
