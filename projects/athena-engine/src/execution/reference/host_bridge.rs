//! Reference → [`ExecutionHost`]（`VmHost`）委托桥。
//!
//! Living `04`：`ReferenceExecutor` 终态只做 host 适配。语义 / extension / scope /
//! binding / collection / index / provider 一律经 [`ExecutionHost`]，本模块只做
//! 帧共享与 outcome 映射。

use athena_types::Result;
use athena_vm::{HostOutcome, ProviderOpId, SlotValue, VmHost};

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
