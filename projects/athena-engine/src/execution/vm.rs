//! engine → `athena-vm` 投影边界（综合体挂接执行运行时）。
//!
//! `athena-engine` 在 `athena-vm` **之上**：本模块只投影 `VmConfig` / 冒烟执行，
//! **禁止**在此实现第二套解释循环或把 M-Graph / 领域算法塞进 VM。

use athena_vm::{CancellationToken, Interpreter, ModuleFingerprint, VmConfig, VmExecutor, VmExit, VmModule};

use crate::{
    execution::ir::{CapturedRoot, ConstantValue, ExecutionModule},
    runtime::session::Session,
};

pub use crate::execution::{
    execution_host::ExecutionHost,
    vm_codegen::{VmCodegenArtifact, try_lower_verified_cfg_module, validate_vm_codegen_subset},
};
pub use athena_vm::{
    ExecutionLease, HostOutcome, Instruction, Interpreter as VmInterpreter, NullHost, ProviderOpId, SemanticOpId, SlotTable, SlotValue,
    VmConfig as EngineVmConfig, VmConstant, VmExecutionContext, VmExit as EngineVmExit, VmHost, VmModule as EngineVmModule,
};

/// 将 module 的 captured Term 根与 Term 常量 pin 到执行期 lease（带 store epoch）。
pub fn pin_module_terms(lease: &mut ExecutionLease, store: &athena_ir::TermStore, module: &ExecutionModule) -> athena_types::Result<()> {
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
/// 领域 Goal 载荷经 module [`crate::execution::ir::ProviderCallDescriptor::payload`] 解析，
/// **禁止** host 侧通道。
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
) -> athena_types::Result<VerifiedVmOutcome> {
    let config = vm_config_from_session(session);
    execute_verified_cfg_on_vm_with_config(session, module, &config)
}

/// 在 `athena-vm` 上执行已验证 CFG 子集 module（显式 [`VmConfig`]）。
///
/// 若 Session 尚无共享执行控制，则用 `config` 安装根控制；嵌套入口继承取消与共享步数预算。
pub fn execute_verified_cfg_on_vm_with_config(
    session: &mut Session,
    module: &crate::execution::ir::ExecutionModule,
    config: &VmConfig,
) -> athena_types::Result<VerifiedVmOutcome> {
    execute_verified_cfg_on_vm_with_config_and_frames(session, module, config, None)
}

/// 同上，但宿主 [`ExecutionHost`] 共享调用方提供的局部 [`ScopeFrame`] 栈。
///
/// 用于参数化复用：同一 `ExecutionModule` 多次执行，只替换帧内绑定，不重新 compile。
pub fn execute_verified_cfg_on_vm_with_frames(
    session: &mut Session,
    module: &crate::execution::ir::ExecutionModule,
    frames: &mut Vec<crate::execution::ScopeFrame>,
) -> athena_types::Result<VerifiedVmOutcome> {
    let config = vm_config_from_session(session);
    execute_verified_cfg_on_vm_with_config_and_frames(session, module, &config, Some(frames))
}

fn execute_verified_cfg_on_vm_with_config_and_frames(
    session: &mut Session,
    module: &crate::execution::ir::ExecutionModule,
    config: &VmConfig,
    frames: Option<&mut Vec<crate::execution::ScopeFrame>>,
) -> athena_types::Result<VerifiedVmOutcome> {
    use crate::runtime::session::SharedExecutionControl;

    let installed = session.begin_shared_execution_root(SharedExecutionControl::from_vm_config(config));
    let effective = vm_config_from_session(session);
    let outcome = (|| {
        let lowered = try_lower_verified_cfg_module(module)?;
        let mut lease = ExecutionLease::new(session.heap().clone());
        pin_module_terms(&mut lease, &session.arena, module)?;
        let mut interpreter = Interpreter::new();
        let exit = {
            let mut host = match frames {
                Some(frames) => {
                    ExecutionHost::with_shared_frames(session, frames, module.provider_calls.clone(), lowered.index_axes.clone())
                }
                None => ExecutionHost::new(session, module.provider_calls.clone(), lowered.index_axes.clone()),
            };
            let mut ctx = VmExecutionContext::with_lease(&mut lease);
            interpreter.execute_with_context(&lowered.module, &effective, &mut host, &mut ctx)?
        };
        // 步数已在解释循环中经共享 `StepBudget` 实时扣减。
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
    })();
    session.end_shared_execution_root(installed);
    outcome
}

/// 将 [`VerifiedVmOutcome`] 物化为 Session 上的 [`athena_types::ResultId`]。
pub fn materialize_verified_vm_outcome(
    session: &mut Session,
    outcome: VerifiedVmOutcome,
    provenance_kind: &'static str,
) -> athena_types::Result<athena_types::ResultId> {
    use crate::execution::reference::symbolic_term_from_value_id;
    use crate::runtime::results::{ComputationResult, CoverageStatus, ResultProvenance};
    use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode};

    // 分轴：residual → 未完成；无 residual 的 IR 宿主返回 → 可信内核完成（Exact/Full）。
    // 真正的搜索候选 / 领域保证由 `SlotValue::Result` 透传，不得在此一律降为 Candidate。
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
        SlotValue::Value(value_id) => {
            let term = symbolic_term_from_value_id(session, value_id)?;
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

/// 从 Session 投影 VM 配置（嵌套入口继承共享取消与剩余预算）。
pub fn vm_config_from_session(session: &Session) -> VmConfig {
    let gc_mode = session.heap().borrow().effective_mode();
    if let Some(ctrl) = session.shared_execution() {
        VmConfig { gc_mode, step_budget: ctrl.step_budget.clone(), cancellation: ctrl.cancellation.clone() }
    } else {
        VmConfig { gc_mode, step_budget: athena_vm::StepBudget::unlimited(), cancellation: CancellationToken::new() }
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
