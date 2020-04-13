//! `athena-vm` 接入边界（骨架）。
//!
//! 完整 `ExecutionModule` 仍由本 crate 的 [`super::ir`] 与 [`super::reference`] 拥有。
//! 本模块只建立 engine → VM 的依赖与配置投影，禁止把 M-Graph / 领域算法搬进 VM。

use athena_vm::{CancellationToken, Interpreter, ModuleFingerprint, VmConfig, VmExit, VmExecutor, VmModule};

use crate::runtime::session::Session;

pub use athena_vm::{
    ExecutionLease, Instruction, Interpreter as VmInterpreter, SlotTable, SlotValue, VmConfig as EngineVmConfig, VmConstant,
    VmExit as EngineVmExit, VmModule as EngineVmModule,
};

/// 从 Session 投影 VM 配置（不复制语义状态）。
pub fn vm_config_from_session(session: &Session) -> VmConfig {
    VmConfig {
        gc_mode: session.heap().borrow().effective_mode(),
        max_steps: None,
        cancellation: CancellationToken::new(),
    }
}

/// 运行 VM 骨架模块（parity / 冒烟）。不替代 [`super::reference::ReferenceExecutor`]。
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
