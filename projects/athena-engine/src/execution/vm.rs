//! engine → `athena-vm` 投影边界（综合体挂接执行运行时）。
//!
//! `athena-engine` 在 `athena-vm` **之上**：本模块只投影 `VmConfig` / 冒烟执行，
//! **禁止**在此实现第二套解释循环或把 M-Graph / 领域算法塞进 VM。

use athena_vm::{CancellationToken, Interpreter, ModuleFingerprint, VmConfig, VmExit, VmExecutor, VmModule};

use crate::runtime::session::Session;

pub use athena_vm::{
    ExecutionLease, HostOutcome, Instruction, Interpreter as VmInterpreter, NullHost, ProviderOpId, SemanticOpId, SlotTable, SlotValue,
    VmConfig as EngineVmConfig, VmConstant, VmExit as EngineVmExit, VmHost, VmModule as EngineVmModule,
};
pub use crate::execution::execution_host::ExecutionHost;
pub use crate::execution::vm_lower::{LoweredBooleanModule, try_lower_linear_boolean_module};

/// 降级并经 [`ExecutionHost`] 在 VM 上执行线性 Boolean module。
///
/// 成功时返回结果槽中的 [`SlotValue`]。降级失败返回诊断（调用方可回退 Reference）。
pub fn execute_linear_boolean_on_vm(
    session: &Session,
    module: &crate::execution::ir::ExecutionModule,
) -> athena_types::Result<SlotValue> {
    let lowered = try_lower_linear_boolean_module(module)?;
    let config = vm_config_from_session(session);
    let mut interpreter = Interpreter::new();
    let mut host = ExecutionHost::new();
    let exit = interpreter.execute_with_host(&lowered.module, &config, &mut host)?;
    match exit {
        VmExit::Returned => {
            let slot = interpreter.last_return_slot().unwrap_or(lowered.result_slot);
            interpreter.slots().get(slot).ok_or_else(|| {
                athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
                    .detail("component", "execute_linear_boolean_on_vm")
                    .detail("reason", "result_slot_empty")
            })
        }
        VmExit::Rejected => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_linear_boolean_on_vm")
            .detail("reason", "rejected")),
        VmExit::Cancelled => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_linear_boolean_on_vm")
            .detail("reason", "cancelled")),
        VmExit::BudgetExceeded => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_linear_boolean_on_vm")
            .detail("reason", "budget_exceeded")),
        VmExit::Suspended => Err(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_linear_boolean_on_vm")
            .detail("reason", "suspended")),
        VmExit::Diagnostic(diagnostic) => Err(diagnostic),
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
