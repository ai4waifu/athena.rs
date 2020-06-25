//! engine → `athena-vm` 投影边界（综合体挂接执行运行时）。
//!
//! `athena-engine` 在 `athena-vm` **之上**：本模块只投影 `VmConfig` / 冒烟执行，
//! **禁止**在此实现第二套解释循环或把 M-Graph / 领域算法塞进 VM。

use athena_vm::{CancellationToken, Interpreter, ModuleFingerprint, VmConfig, VmExit, VmExecutor, VmModule};

use crate::execution::ir::{CapturedRoot, ConstantValue, ExecutionModule};
use crate::runtime::session::Session;

pub use athena_vm::{
    ExecutionLease, HostOutcome, Instruction, Interpreter as VmInterpreter, NullHost, ProviderOpId, SemanticOpId, SlotTable, SlotValue,
    VmConfig as EngineVmConfig, VmConstant, VmExecutionContext, VmExit as EngineVmExit, VmHost, VmModule as EngineVmModule,
};
pub use crate::execution::execution_host::ExecutionHost;
pub use crate::execution::vm_codegen::{VmCodegenArtifact, try_lower_verified_cfg_module, validate_vm_codegen_subset};

/// 将 module 的 captured Term 根与 Term 常量 pin 到执行期 lease（带 store epoch）。
pub fn pin_module_terms(
    lease: &mut ExecutionLease,
    store: &athena_ir::TermStore,
    module: &ExecutionModule,
) -> athena_types::Result<()> {
    for root in &module.captured_roots {
        if let CapturedRoot::Term(term_ref) = root {
            // Re-validate against current store epoch before pinning.
            let _ = store.check_ref(*term_ref)?;
            lease.register_term(*term_ref);
        }
    }
    for constant in &module.constants {
        if let ConstantValue::Term(term) = constant {
            let term_ref = store.term_ref(*term).ok_or_else(|| {
                athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                    .detail("component", "pin_module_terms")
                    .detail("reason", "term_out_of_range")
                    .detail("term", term.0)
            })?;
            lease.register_term(term_ref);
        }
    }
    Ok(())
}

/// 降级并经 [`ExecutionHost`] 在 VM 上执行 verified CFG 子集 module。
///
/// `pending_domain` 供首条 `CallProvider` 消费（与 Reference 路径同合同）。
/// Verified CFG 在 `athena-vm` 上的执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedVmOutcome {
    /// 返回槽值。
    pub value: SlotValue,
    /// 是否出现过 host Residual（未知 / 未求值覆盖）。
    pub residual: bool,
}

/// 在 `athena-vm` 上执行已验证 CFG 子集 module（Session 默认预算 / 取消）。
pub fn execute_verified_cfg_on_vm(
    session: &mut Session,
    module: &crate::execution::ir::ExecutionModule,
    pending_domain: Option<crate::domains::dispatch::DomainRequest>,
) -> athena_types::Result<VerifiedVmOutcome> {
    let config = vm_config_from_session(session);
    execute_verified_cfg_on_vm_with_config(session, module, pending_domain, &config)
}

/// 在 `athena-vm` 上执行已验证 CFG 子集 module（显式 [`VmConfig`]）。
pub fn execute_verified_cfg_on_vm_with_config(
    session: &mut Session,
    module: &crate::execution::ir::ExecutionModule,
    pending_domain: Option<crate::domains::dispatch::DomainRequest>,
    config: &VmConfig,
) -> athena_types::Result<VerifiedVmOutcome> {
    let lowered = try_lower_verified_cfg_module(module)?;
    let mut lease = ExecutionLease::new(session.heap().clone());
    pin_module_terms(&mut lease, &session.arena, module)?;
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new(
        session,
        module.provider_calls.clone(),
        pending_domain,
        lowered.index_axes,
    );
    let exit = {
        let mut ctx = VmExecutionContext::with_lease(&mut lease);
        interpreter.execute_with_context(&lowered.module, config, &mut host, &mut ctx)?
    };
    let residual = interpreter.saw_host_residual();
    drop(lease);
    match exit {
        VmExit::Returned => {
            let slot = interpreter.last_return_slot().unwrap_or(lowered.result_slot);
            let value = interpreter.slots().get(slot).ok_or_else(|| {
                athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                    .detail("component", "execute_verified_cfg_on_vm")
                    .detail("reason", "result_slot_empty")
            })?;
            Ok(VerifiedVmOutcome { value, residual })
        }
        VmExit::Rejected => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_verified_cfg_on_vm")
            .detail("reason", "rejected")),
        VmExit::Cancelled => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_verified_cfg_on_vm")
            .detail("reason", "cancelled")),
        VmExit::BudgetExceeded => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_verified_cfg_on_vm")
            .detail("reason", "budget_exceeded")),
        VmExit::Suspended => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_verified_cfg_on_vm")
            .detail("reason", "suspended")),
        VmExit::Diagnostic(diagnostic) => Err(diagnostic),
    }
}

/// 将 [`VerifiedVmOutcome`] 物化为 Session 上的 [`athena_types::ResultId`]。
pub fn materialize_verified_vm_outcome(
    session: &mut Session,
    outcome: VerifiedVmOutcome,
    provenance_kind: &'static str,
) -> athena_types::Result<athena_types::ResultId> {
    use crate::runtime::results::{ComputationResult, CoverageStatus, ResultProvenance};
    use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode};

    let (status, coverage) = if outcome.residual {
        (ComputationStatus::Unknown, CoverageStatus::Partial)
    } else {
        (ComputationStatus::Exact, CoverageStatus::Full)
    };
    match outcome.value {
        SlotValue::Result(result_id) => Ok(result_id),
        SlotValue::Boolean(value) => {
            let term = session.builder().boolean(value, Default::default());
            let value_id = session.insert_symbolic_value(term);
            let result = ComputationResult::with_status(status, coverage)
                .with_value(value_id)
                .with_symbolic_term(term)
                .with_provenance(ResultProvenance::kind(provenance_kind));
            Ok(session.insert_result(result))
        }
        SlotValue::Term(term) => {
            let term_ref = session.arena.term_ref(term).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "materialize_verified_vm_outcome")
                    .detail("reason", "vm_term_out_of_range")
            })?;
            let term = session.arena.check_ref(term_ref)?;
            let value_id = session.insert_symbolic_value(term);
            let result = ComputationResult::with_status(status, coverage)
                .with_value(value_id)
                .with_symbolic_term(term)
                .with_provenance(ResultProvenance::kind(provenance_kind));
            Ok(session.insert_result(result))
        }
        SlotValue::Symbol(symbol) => {
            let term = session.builder().symbol_id(symbol, Default::default());
            let value_id = session.insert_symbolic_value(term);
            let result = ComputationResult::with_status(status, coverage)
                .with_value(value_id)
                .with_symbolic_term(term)
                .with_provenance(ResultProvenance::kind(provenance_kind));
            Ok(session.insert_result(result))
        }
        SlotValue::Unit | SlotValue::Scope(_) => {
            let term = session.builder().null(Default::default());
            let value_id = session.insert_symbolic_value(term);
            let result = ComputationResult::with_status(status, coverage)
                .with_value(value_id)
                .with_symbolic_term(term)
                .with_provenance(ResultProvenance::kind(provenance_kind));
            Ok(session.insert_result(result))
        }
        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "materialize_verified_vm_outcome")
            .detail("reason", "vm_unexpected_slot_kind")),
    }
}

/// 从 Session 投影 VM 配置（不复制语义状态）。
pub fn vm_config_from_session(session: &Session) -> VmConfig {
    VmConfig {
        gc_mode: session.heap().borrow().effective_mode(),
        max_steps: None,
        cancellation: CancellationToken::new(),
    }
}

/// 运行 VM 模块（parity / 冒烟）。生产 SSA 路径终态应走 VM 解释循环 + host，而非本函数替代。
pub fn execute_vm_module(session: &Session, module: &VmModule) -> athena_types::Result<VmExit> {
    let config = vm_config_from_session(session);
    let mut interpreter = Interpreter::new();
    interpreter.execute(module, &config)
}

/// 构造与 engine IR 指纹域隔离的空返回模块（骨架 parity）。
pub fn empty_vm_module() -> VmModule {
    VmModule::empty_return()
}

/// 暴露指纹类型，便于后续与 engine IR `ModuleFingerprint` 对照。
pub type VmModuleFingerprint = ModuleFingerprint;
