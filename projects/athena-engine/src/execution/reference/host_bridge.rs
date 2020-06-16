//! Reference → [`ExecutionHost`]（`VmHost`）委托桥。
//!
//! Living `04`：`ReferenceExecutor` 终态只做 host 适配。本模块把已由
//! [`ExecutionHost`] 覆盖、且不依赖 Reference 专有语义（数值 truthiness /
//! iterator 特例残留路径）的算子，先走统一 host，避免第二套循环继续复制语义。

use athena_ir::SemanticOperator;
use athena_types::Result;
use athena_vm::{HostOutcome, ProviderOpId, SemanticOpId, SlotValue, VmHost};

use crate::domains::dispatch::DomainRequest;
use crate::execution::environment::ScopeFrame;
use crate::execution::execution_host::ExecutionHost;
use crate::execution::ir::{ExecutionModule, ProviderCallDescriptor, ProviderCallId};
use crate::runtime::session::Session;

/// 将 host 结果映射为槽值（硬失败透传）。
pub(crate) fn host_outcome_to_slot(outcome: HostOutcome) -> Result<SlotValue> {
    match outcome {
        HostOutcome::Value(value) | HostOutcome::Residual(value) => Ok(value),
        HostOutcome::SoftInvalid { value, .. } => Ok(value),
        HostOutcome::Diagnostic(diagnostic) => Err(diagnostic),
    }
}

/// 将 host 结果映射为槽值，并捕获软 Invalid 诊断。
pub(crate) fn host_outcome_to_slot_capturing_invalid(
    outcome: HostOutcome,
    invalid: &mut Option<athena_types::Diagnostic>,
) -> Result<SlotValue> {
    match outcome {
        HostOutcome::Value(value) | HostOutcome::Residual(value) => Ok(value),
        HostOutcome::SoftInvalid { value, diagnostic } => {
            *invalid = Some(diagnostic);
            Ok(value)
        }
        HostOutcome::Diagnostic(diagnostic) => Err(diagnostic),
    }
}

/// 经共享帧栈构造 host，委托 scope / binding / provider / collection。
pub(crate) fn host_with_shared_frames<'a>(
    session: &'a mut Session,
    frames: &'a mut Vec<ScopeFrame>,
    provider_calls: Vec<ProviderCallDescriptor>,
    pending_domain: Option<DomainRequest>,
) -> ExecutionHost<'a> {
    ExecutionHost::with_shared_frames(session, frames, provider_calls, pending_domain, Vec::new())
}

/// 经共享帧与一次性 index axes 表构造 host。
pub(crate) fn host_with_shared_frames_and_axes<'a>(
    session: &'a mut Session,
    frames: &'a mut Vec<ScopeFrame>,
    index_axes: Vec<Vec<athena_types::IndexSpec>>,
) -> ExecutionHost<'a> {
    ExecutionHost::with_shared_frames(session, frames, Vec::new(), None, index_axes)
}

/// Reference `CallProvider` → [`ExecutionHost::call_provider`]。
///
/// 缺 domain 时 Reference 合同是软失败（`unsupported` + `Unit`），不是硬诊断。
pub(crate) fn delegate_call_provider(
    session: &mut Session,
    frames: &mut Vec<ScopeFrame>,
    module: &ExecutionModule,
    pending_domain: Option<DomainRequest>,
    call: ProviderCallId,
) -> Result<(SlotValue, bool)> {
    let mut host = host_with_shared_frames(session, frames, module.provider_calls.clone(), pending_domain);
    match host.call_provider(ProviderOpId(call.0), &[])? {
        HostOutcome::Value(value) | HostOutcome::Residual(value) => Ok((value, false)),
        HostOutcome::SoftInvalid { diagnostic, .. } | HostOutcome::Diagnostic(diagnostic) => {
            let reason = diagnostic
                .details
                .get("reason")
                .map(|value| value.to_string())
                .unwrap_or_default();
            if reason == "provider_domain_missing" {
                Ok((SlotValue::Unit, true))
            } else {
                Err(diagnostic)
            }
        }
    }
}

/// 是否可安全委托给 [`ExecutionHost::apply_semantic`]。
fn is_host_delegable(op: SemanticOperator, args: &[SlotValue]) -> bool {
    match op {
        SemanticOperator::Not | SemanticOperator::TrueQ | SemanticOperator::And | SemanticOperator::Or => {
            args.iter().all(|slot| matches!(slot, SlotValue::Boolean(_)))
        }
        SemanticOperator::Add
        | SemanticOperator::Multiply
        | SemanticOperator::Subtract
        | SemanticOperator::Negate
        | SemanticOperator::Divide
        | SemanticOperator::Power
        | SemanticOperator::ElementwiseMultiply
        | SemanticOperator::ElementwiseDivide
        | SemanticOperator::ElementwisePower
        | SemanticOperator::Less
        | SemanticOperator::Greater
        | SemanticOperator::LessEqual
        | SemanticOperator::GreaterEqual
        | SemanticOperator::Abs
        | SemanticOperator::Length
        | SemanticOperator::First
        | SemanticOperator::Rest
        | SemanticOperator::Factorial
        | SemanticOperator::Sqrt
        | SemanticOperator::Join
        | SemanticOperator::Range
        | SemanticOperator::Size
        | SemanticOperator::Determinant
        | SemanticOperator::Zeros
        | SemanticOperator::Ones
        | SemanticOperator::Eye
        | SemanticOperator::Equal
        | SemanticOperator::Unequal
        | SemanticOperator::Identical => true,
        // 一元 `Sum` 可走 host；二元 iterator fold 仍在 Reference（`table_values`）。
        SemanticOperator::Sum => args.len() == 1,
        // Map / Product / Apply / 规则重写等未进 host。
        _ => false,
    }
}

/// 尝试经 [`ExecutionHost`] 求值。`None` 表示回退 Reference 本地路径。
pub(crate) fn try_delegate_semantic_to_host(
    session: &mut Session,
    op: SemanticOperator,
    args: &[SlotValue],
    invalid: &mut Option<athena_types::Diagnostic>,
) -> Result<Option<SlotValue>> {
    if !is_host_delegable(op, args) {
        return Ok(None);
    }
    let mut host = ExecutionHost::new(session, Vec::new(), None, Vec::new());
    match host.apply_semantic(SemanticOpId(op.discriminant()), args)? {
        HostOutcome::Value(value) | HostOutcome::Residual(value) => Ok(Some(value)),
        HostOutcome::SoftInvalid { value, diagnostic } => {
            *invalid = Some(diagnostic);
            Ok(Some(value))
        }
        HostOutcome::Diagnostic(diagnostic) => {
            let reason = diagnostic
                .details
                .get("reason")
                .map(|value| value.to_string())
                .unwrap_or_default();
            match reason.as_str() {
                "apply_semantic_deferred_to_reference"
                | "and_expects_boolean"
                | "or_expects_boolean"
                | "not_expects_boolean"
                | "trueq_expects_boolean" => Ok(None),
                _ => Err(diagnostic),
            }
        }
    }
}
