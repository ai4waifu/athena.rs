//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期覆盖句柄级 Boolean 语义（`Not` / `And` / `Or` / `TrueQ` / `Equal` / `Unequal`）。
//! 完整 SSA / Term 语义仍在 [`crate::execution::reference`]。终态由 Reference 循环迁入 VM 后扩展本 host。

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

    fn unsupported(op: SemanticOpId) -> HostOutcome {
        HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "apply_semantic_deferred_to_reference")
                .detail("op", op.0),
        )
    }

    fn expect_boolean(args: &[SlotValue], index: usize, reason: &'static str) -> Result<core::result::Result<bool, HostOutcome>> {
        match args.get(index).copied() {
            Some(SlotValue::Boolean(v)) => Ok(Ok(v)),
            Some(_) | None => Ok(Err(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", reason),
            ))),
        }
    }
}

impl VmHost for ExecutionHost {
    fn apply_semantic(&mut self, op: SemanticOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        if op.0 == SemanticOperator::Not.discriminant() {
            return match Self::expect_boolean(args, 0, "not_expects_boolean")? {
                Ok(v) => Ok(HostOutcome::Value(SlotValue::Boolean(!v))),
                Err(outcome) => Ok(outcome),
            };
        }
        if op.0 == SemanticOperator::TrueQ.discriminant() {
            return match Self::expect_boolean(args, 0, "trueq_expects_boolean")? {
                Ok(v) => Ok(HostOutcome::Value(SlotValue::Boolean(v))),
                Err(outcome) => Ok(outcome),
            };
        }
        if op.0 == SemanticOperator::And.discriminant() {
            let left = match Self::expect_boolean(args, 0, "and_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            let right = match Self::expect_boolean(args, 1, "and_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            return Ok(HostOutcome::Value(SlotValue::Boolean(left && right)));
        }
        if op.0 == SemanticOperator::Or.discriminant() {
            let left = match Self::expect_boolean(args, 0, "or_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            let right = match Self::expect_boolean(args, 1, "or_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            return Ok(HostOutcome::Value(SlotValue::Boolean(left || right)));
        }
        if op.0 == SemanticOperator::Equal.discriminant() || op.0 == SemanticOperator::Unequal.discriminant() {
            let left = match Self::expect_boolean(args, 0, "compare_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            let right = match Self::expect_boolean(args, 1, "compare_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            let eq = left == right;
            let out = if op.0 == SemanticOperator::Equal.discriminant() { eq } else { !eq };
            return Ok(HostOutcome::Value(SlotValue::Boolean(out)));
        }
        Ok(Self::unsupported(op))
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
